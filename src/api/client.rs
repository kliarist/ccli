//! Confluence Data Center API client and PAT validation probe.
//!
//! Locked decisions implemented:
//! - D-08 tokio + reqwest async
//! - PAT auth via `Authorization: Bearer <token>` header (RESEARCH.md Pattern 4)
//! - INIT-04 error categories surfaced via AppError (Auth / Network / Api)
//! - Open Question 1: CCLI_INSECURE=1 enables `danger_accept_invalid_certs(true)`
//!   to support Confluence DC instances with self-signed or internal-CA certs
//!   (RESEARCH.md Pitfall 3). Default is strict TLS.
//!
//! Security:
//! - The token NEVER appears in tracing logs. `#[instrument(skip(token))]` is
//!   applied to test_connection. The internal `build_reqwest_client` helper
//!   takes the token as a `&str` and writes it only into the
//!   `Authorization` header; the header value is not logged.

use std::time::Duration;

use reqwest::{header, Client as ReqwestClient};
use tracing::{debug, instrument};

use crate::api::error::AppError;
use crate::config::Config;

/// Long-lived API client. Holds a single reqwest::Client (connection pool).
/// Created once per command invocation, reused across all calls.
pub struct Client {
    inner: ReqwestClient,
    base_url: String,
    accepts_invalid_certs: bool,
}

impl Client {
    /// Build a Client from the loaded Config.
    pub fn new(config: &Config) -> Result<Self, AppError> {
        let accepts_invalid_certs = read_insecure_env();
        let inner = build_reqwest_client(&config.token, accepts_invalid_certs)?;
        Ok(Self {
            inner,
            base_url: config.url.trim_end_matches('/').to_string(),
            accepts_invalid_certs,
        })
    }

    /// Whether this client was built with `danger_accept_invalid_certs(true)`.
    /// Public read-only accessor — useful for tests and for warning the user
    /// in debug logs when CCLI_INSECURE is active.
    pub fn accepts_invalid_certs(&self) -> bool {
        self.accepts_invalid_certs
    }

    /// Borrow the underlying reqwest client (used by Phase 2+ API call modules).
    pub fn inner(&self) -> &ReqwestClient {
        &self.inner
    }

    /// Borrow the configured base URL (no trailing slash).
    pub fn base_url(&self) -> &str {
        &self.base_url
    }
}

/// Read CCLI_INSECURE env var. Only the literal value `"1"` enables insecure mode.
fn read_insecure_env() -> bool {
    std::env::var("CCLI_INSECURE").map(|v| v == "1").unwrap_or(false)
}

/// Construct a reqwest::Client with PAT auth header, 30s timeout, and optional
/// `danger_accept_invalid_certs(true)` for self-signed certs.
fn build_reqwest_client(token: &str, accept_invalid_certs: bool) -> Result<ReqwestClient, AppError> {
    let mut headers = header::HeaderMap::new();
    let auth_value = header::HeaderValue::from_str(&format!("Bearer {}", token))
        .map_err(|e| AppError::Config(format!("Invalid token characters: {}", e)))?;
    headers.insert(header::AUTHORIZATION, auth_value);

    let mut builder = ReqwestClient::builder()
        .default_headers(headers)
        .timeout(Duration::from_secs(30));

    if accept_invalid_certs {
        builder = builder.danger_accept_invalid_certs(true);
    }

    builder
        .build()
        .map_err(|e| AppError::Network(format!("Failed to build HTTP client: {}", e)))
}

/// Probe `GET /rest/api/user/current` to validate the PAT.
///
/// This endpoint is the only reliable PAT validation probe on Confluence DC 9.x —
/// other endpoints suffer from CONFSERVER-87540 returning 200 with empty body
/// for invalid tokens (RESEARCH.md Pitfall 2).
///
/// Returns the authenticated user's `displayName` on success.
#[instrument(skip(token))]
pub async fn test_connection(base_url: &str, token: &str) -> Result<String, AppError> {
    let trimmed = base_url.trim_end_matches('/');
    let url = format!("{}/rest/api/user/current", trimmed);
    debug!("PAT validation probe: GET {}", url); // never log the token value

    let client = build_reqwest_client(token, read_insecure_env())?;

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| {
            if e.is_connect() || e.is_timeout() {
                AppError::Network(format!("Cannot reach {}: {}", trimmed, e))
            } else {
                AppError::Network(e.to_string())
            }
        })?;

    match resp.status().as_u16() {
        200 => {
            // W-04: treat empty/missing displayName as auth failure (Pitfall 2 / CONFSERVER-87540).
            let body: serde_json::Value = resp.json().await.unwrap_or_default();
            match body["displayName"].as_str() {
                Some(name) if !name.is_empty() => Ok(name.to_string()),
                _ => Err(AppError::Auth(
                    "Server returned 200 but no user identity. Token may be invalid.".to_string(),
                )),
            }
        }
        401 => Err(AppError::Auth(
            "Invalid or expired token. Check your PAT.".to_string(),
        )),
        403 => Err(AppError::Auth(
            "Authenticated but access denied. Check your PAT permissions.".to_string(),
        )),
        status => Err(AppError::Api(format!("Unexpected HTTP {}", status))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::prelude::*;
    use std::sync::Mutex;

    // Serialize tests that mutate CCLI_INSECURE — Rust runs unit tests in parallel.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn fixture_config(url: &str) -> Config {
        Config { url: url.to_string(), token: "tok-xyz".into() }
    }

    #[test]
    fn client_new_ok_with_default_strict_tls() {
        let _g = ENV_LOCK.lock().unwrap();
        std::env::remove_var("CCLI_INSECURE");
        let c = Client::new(&fixture_config("https://example.com")).expect("new");
        assert!(!c.accepts_invalid_certs(), "default must be strict TLS");
        assert_eq!(c.base_url(), "https://example.com");
    }

    #[test]
    fn client_new_with_ccli_insecure_enables_invalid_certs() {
        let _g = ENV_LOCK.lock().unwrap();
        std::env::set_var("CCLI_INSECURE", "1");
        let c = Client::new(&fixture_config("https://example.com")).expect("new");
        assert!(c.accepts_invalid_certs(), "CCLI_INSECURE=1 must enable danger_accept_invalid_certs");
        std::env::remove_var("CCLI_INSECURE");
    }

    #[test]
    fn client_new_ignores_ccli_insecure_other_values() {
        let _g = ENV_LOCK.lock().unwrap();
        std::env::set_var("CCLI_INSECURE", "true"); // not "1" — ignored
        let c = Client::new(&fixture_config("https://example.com")).expect("new");
        assert!(!c.accepts_invalid_certs());
        std::env::remove_var("CCLI_INSECURE");
    }

    #[test]
    fn base_url_trims_trailing_slashes() {
        let _g = ENV_LOCK.lock().unwrap();
        std::env::remove_var("CCLI_INSECURE");
        let c = Client::new(&fixture_config("https://example.com///")).expect("new");
        assert_eq!(c.base_url(), "https://example.com");
    }

    #[tokio::test]
    async fn test_connection_returns_display_name_on_200() {
        let server = MockServer::start();
        let _m = server.mock(|when, then| {
            when.method(GET).path("/rest/api/user/current");
            then.status(200).json_body(serde_json::json!({"displayName": "Alice"}));
        });
        let result = test_connection(&server.base_url(), "valid-tok").await;
        assert_eq!(result.unwrap(), "Alice");
    }

    #[tokio::test]
    async fn test_connection_returns_auth_error_on_401() {
        let server = MockServer::start();
        let _m = server.mock(|when, then| {
            when.method(GET).path("/rest/api/user/current");
            then.status(401);
        });
        let result = test_connection(&server.base_url(), "bad").await;
        match result {
            Err(AppError::Auth(msg)) => assert!(msg.to_lowercase().contains("invalid") || msg.to_lowercase().contains("expired")),
            other => panic!("expected AppError::Auth, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_connection_returns_auth_error_on_403() {
        let server = MockServer::start();
        let _m = server.mock(|when, then| {
            when.method(GET).path("/rest/api/user/current");
            then.status(403);
        });
        let result = test_connection(&server.base_url(), "limited").await;
        match result {
            Err(AppError::Auth(msg)) => assert!(msg.to_lowercase().contains("access denied") || msg.to_lowercase().contains("permission")),
            other => panic!("expected AppError::Auth, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_connection_returns_api_error_on_500() {
        let server = MockServer::start();
        let _m = server.mock(|when, then| {
            when.method(GET).path("/rest/api/user/current");
            then.status(500);
        });
        let result = test_connection(&server.base_url(), "tok").await;
        match result {
            Err(AppError::Api(msg)) => assert!(msg.contains("500")),
            other => panic!("expected AppError::Api, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_connection_strips_trailing_slash_from_base_url() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(GET).path("/rest/api/user/current");
            then.status(200).json_body(serde_json::json!({"displayName": "Bob"}));
        });
        // Pass a base URL with trailing slashes; the function should normalize.
        let mut url = server.base_url();
        url.push_str("///");
        let result = test_connection(&url, "tok").await;
        mock.assert();
        assert_eq!(result.unwrap(), "Bob");
    }

    #[tokio::test]
    async fn test_connection_returns_network_error_on_unreachable_host() {
        // W-03: Use localhost port 1 (reserved, immediate TCP connection-refused, no DNS lookup).
        let result = test_connection("http://127.0.0.1:1", "tok").await;
        match result {
            Err(AppError::Network(_)) => {} // ok
            other => panic!("expected AppError::Network, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_connection_returns_auth_error_on_200_with_empty_body() {
        // W-04: Pitfall 2 (CONFSERVER-87540) — 200 + empty body must not silently succeed.
        let server = MockServer::start();
        let _m = server.mock(|when, then| {
            when.method(GET).path("/rest/api/user/current");
            then.status(200).body("");
        });
        let result = test_connection(&server.base_url(), "invalid-tok").await;
        match result {
            Err(AppError::Auth(_)) => {}
            other => panic!("expected AppError::Auth on empty 200 body, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_connection_returns_auth_error_on_200_with_missing_display_name() {
        // W-04: 200 with `{}` (no displayName) must also fail (Pitfall 2 mitigation).
        let server = MockServer::start();
        let _m = server.mock(|when, then| {
            when.method(GET).path("/rest/api/user/current");
            then.status(200).json_body(serde_json::json!({}));
        });
        let result = test_connection(&server.base_url(), "tok").await;
        match result {
            Err(AppError::Auth(_)) => {}
            other => panic!("expected AppError::Auth on 200 with missing displayName, got {:?}", other),
        }
    }
}
