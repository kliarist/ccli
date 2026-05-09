//! Confluence REST API client with Cloud and Data Center / Server support.
//!
//! Auth schemes:
//! - Confluence Cloud (*.atlassian.net): Basic Auth — `Authorization: Basic base64(email:token)`
//! - Confluence DC / Server: Bearer PAT — `Authorization: Bearer <token>`
//!
//! Security:
//! - Token and email values never appear in tracing logs.
//! - CCLI_INSECURE=1 enables `danger_accept_invalid_certs(true)` for self-signed certs.

use std::time::Duration;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use reqwest::{header, Client as ReqwestClient};
use tracing::{debug, instrument};

use crate::api::error::AppError;
use crate::config::Config;

/// Returns true when `url` points to a Confluence Cloud instance (*.atlassian.net).
pub fn is_cloud_url(url: &str) -> bool {
    let without_scheme = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url);
    let host = without_scheme.split('/').next().unwrap_or("");
    host.ends_with(".atlassian.net")
}

/// Normalize a Confluence URL to the canonical base used for all API calls.
///
/// Cloud URLs: strip any path after the host and append `/wiki`.
///   `https://org.atlassian.net/wiki/home` → `https://org.atlassian.net/wiki`
/// DC/Server URLs: strip trailing slashes only.
pub fn normalize_url(url: &str) -> String {
    let trimmed = url.trim_end_matches('/');
    if !is_cloud_url(trimmed) {
        return trimmed.to_string();
    }
    let scheme = if trimmed.starts_with("https://") {
        "https"
    } else {
        "http"
    };
    let without_scheme = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
        .unwrap_or(trimmed);
    let host = without_scheme.split('/').next().unwrap_or(without_scheme);
    format!("{}://{}/wiki", scheme, host)
}

/// Long-lived API client. Holds a single reqwest::Client (connection pool).
/// Created once per command invocation, reused across all calls.
/// Clone is cheap — reqwest::Client shares the underlying connection pool (Arc-backed).
#[derive(Clone)]
pub struct Client {
    inner: ReqwestClient,
    base_url: String,
    #[allow(dead_code)]
    accepts_invalid_certs: bool,
}

impl Client {
    /// Build a Client from the loaded Config.
    pub fn new(config: &Config) -> Result<Self, AppError> {
        let accepts_invalid_certs = read_insecure_env();
        let inner = build_reqwest_client(
            &config.token,
            config.email.as_deref(),
            accepts_invalid_certs,
        )?;
        Ok(Self {
            inner,
            base_url: config.url.trim_end_matches('/').to_string(),
            accepts_invalid_certs,
        })
    }

    /// Whether this client was built with `danger_accept_invalid_certs(true)`.
    /// Public read-only accessor — useful for tests and for warning the user
    /// in debug logs when CCLI_INSECURE is active.
    #[allow(dead_code)]
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
    std::env::var("CCLI_INSECURE")
        .map(|v| v == "1")
        .unwrap_or(false)
}

/// Construct a reqwest::Client with the appropriate auth header, 30s timeout, and
/// optional `danger_accept_invalid_certs(true)` for self-signed certs.
///
/// `email = Some(...)` → Basic Auth (Confluence Cloud)
/// `email = None`      → Bearer PAT (Confluence DC / Server)
fn build_reqwest_client(
    token: &str,
    email: Option<&str>,
    accept_invalid_certs: bool,
) -> Result<ReqwestClient, AppError> {
    let mut headers = header::HeaderMap::new();
    let auth_value = match email {
        Some(e) => {
            let encoded = STANDARD.encode(format!("{}:{}", e, token));
            header::HeaderValue::from_str(&format!("Basic {}", encoded)).map_err(|err| {
                AppError::Config(format!("Invalid credential characters: {}", err))
            })?
        }
        None => header::HeaderValue::from_str(&format!("Bearer {}", token))
            .map_err(|err| AppError::Config(format!("Invalid token characters: {}", err)))?,
    };
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

/// Probe `GET /rest/api/user/current` to validate credentials.
///
/// For Cloud (`email = Some`): uses Basic Auth; base_url must be the normalized
/// `https://org.atlassian.net/wiki` form so the path resolves correctly.
/// For DC/Server (`email = None`): uses Bearer PAT.
///
/// Returns the authenticated user's `displayName` on success.
#[instrument(skip(token, email))]
pub async fn test_connection(
    base_url: &str,
    token: &str,
    email: Option<&str>,
) -> Result<String, AppError> {
    let trimmed = base_url.trim_end_matches('/');
    let url = format!("{}/rest/api/user/current", trimmed);
    debug!("auth probe: GET {}", url);

    let client = build_reqwest_client(token, email, read_insecure_env())?;

    let resp = client.get(&url).send().await.map_err(|e| {
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
        401 => {
            let msg = if email.is_some() {
                "Invalid email or API token. Check your Atlassian API token."
            } else {
                "Invalid or expired token. Check your PAT."
            };
            Err(AppError::Auth(msg.to_string()))
        }
        403 => {
            let msg = if email.is_some() {
                "Authenticated but access denied. Ensure your API token has Confluence access."
            } else {
                "Authenticated but access denied. Check your PAT permissions."
            };
            Err(AppError::Auth(msg.to_string()))
        }
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
        Config {
            url: url.to_string(),
            token: "tok-xyz".into(),
            email: None,
        }
    }

    fn fixture_cloud_config(url: &str) -> Config {
        Config {
            url: url.to_string(),
            token: "ATATT3xtoken".into(),
            email: Some("user@example.com".into()),
        }
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
        assert!(
            c.accepts_invalid_certs(),
            "CCLI_INSECURE=1 must enable danger_accept_invalid_certs"
        );
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
            then.status(200)
                .json_body(serde_json::json!({"displayName": "Alice"}));
        });
        let result = test_connection(&server.base_url(), "valid-tok", None).await;
        assert_eq!(result.unwrap(), "Alice");
    }

    #[tokio::test]
    async fn test_connection_returns_auth_error_on_401() {
        let server = MockServer::start();
        let _m = server.mock(|when, then| {
            when.method(GET).path("/rest/api/user/current");
            then.status(401);
        });
        let result = test_connection(&server.base_url(), "bad", None).await;
        match result {
            Err(AppError::Auth(msg)) => assert!(
                msg.to_lowercase().contains("invalid") || msg.to_lowercase().contains("expired")
            ),
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
        let result = test_connection(&server.base_url(), "limited", None).await;
        match result {
            Err(AppError::Auth(msg)) => assert!(
                msg.to_lowercase().contains("access denied")
                    || msg.to_lowercase().contains("permission")
            ),
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
        let result = test_connection(&server.base_url(), "tok", None).await;
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
            then.status(200)
                .json_body(serde_json::json!({"displayName": "Bob"}));
        });
        // Pass a base URL with trailing slashes; the function should normalize.
        let mut url = server.base_url();
        url.push_str("///");
        let result = test_connection(&url, "tok", None).await;
        mock.assert();
        assert_eq!(result.unwrap(), "Bob");
    }

    #[tokio::test]
    async fn test_connection_returns_network_error_on_unreachable_host() {
        // W-03: Use localhost port 1 (reserved, immediate TCP connection-refused, no DNS lookup).
        let result = test_connection("http://127.0.0.1:1", "tok", None).await;
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
        let result = test_connection(&server.base_url(), "invalid-tok", None).await;
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
        let result = test_connection(&server.base_url(), "tok", None).await;
        match result {
            Err(AppError::Auth(_)) => {}
            other => panic!(
                "expected AppError::Auth on 200 with missing displayName, got {:?}",
                other
            ),
        }
    }

    // --- is_cloud_url tests ---

    #[test]
    fn is_cloud_url_atlassian_net_is_cloud() {
        assert!(is_cloud_url("https://myorg.atlassian.net/wiki"));
        assert!(is_cloud_url("https://myorg.atlassian.net"));
        assert!(is_cloud_url("https://myorg.atlassian.net/wiki/home"));
    }

    #[test]
    fn is_cloud_url_dc_hosts_are_not_cloud() {
        assert!(!is_cloud_url("https://confluence.example.com"));
        assert!(!is_cloud_url("http://internal.company.net/confluence"));
        assert!(!is_cloud_url("https://notatlassian.net"));
    }

    // --- normalize_url tests ---

    #[test]
    fn normalize_url_cloud_strips_path_after_wiki() {
        assert_eq!(
            normalize_url("https://myorg.atlassian.net/wiki/home"),
            "https://myorg.atlassian.net/wiki"
        );
        assert_eq!(
            normalize_url("https://myorg.atlassian.net/wiki/spaces/~abc"),
            "https://myorg.atlassian.net/wiki"
        );
    }

    #[test]
    fn normalize_url_cloud_with_no_path_appends_wiki() {
        assert_eq!(
            normalize_url("https://myorg.atlassian.net"),
            "https://myorg.atlassian.net/wiki"
        );
    }

    #[test]
    fn normalize_url_cloud_strips_trailing_slashes() {
        assert_eq!(
            normalize_url("https://myorg.atlassian.net/wiki/"),
            "https://myorg.atlassian.net/wiki"
        );
    }

    #[test]
    fn normalize_url_dc_trims_trailing_slashes_only() {
        assert_eq!(
            normalize_url("https://confluence.example.com/"),
            "https://confluence.example.com"
        );
        assert_eq!(
            normalize_url("https://confluence.example.com"),
            "https://confluence.example.com"
        );
    }

    // --- Cloud Client construction test ---

    #[test]
    fn client_new_cloud_config_builds_ok() {
        let _g = ENV_LOCK.lock().unwrap();
        std::env::remove_var("CCLI_INSECURE");
        let cfg = fixture_cloud_config("https://myorg.atlassian.net/wiki");
        let c = Client::new(&cfg).expect("new with cloud config");
        assert_eq!(c.base_url(), "https://myorg.atlassian.net/wiki");
    }

    // --- Cloud test_connection with Basic Auth ---

    #[tokio::test]
    async fn test_connection_cloud_returns_display_name_on_200() {
        let server = MockServer::start();
        let _m = server.mock(|when, then| {
            when.method(GET).path("/rest/api/user/current");
            then.status(200)
                .json_body(serde_json::json!({"displayName": "Cloud User"}));
        });
        let result =
            test_connection(&server.base_url(), "ATATT3xtoken", Some("user@example.com")).await;
        assert_eq!(result.unwrap(), "Cloud User");
    }

    #[tokio::test]
    async fn test_connection_cloud_returns_cloud_specific_error_on_401() {
        let server = MockServer::start();
        let _m = server.mock(|when, then| {
            when.method(GET).path("/rest/api/user/current");
            then.status(401);
        });
        let result =
            test_connection(&server.base_url(), "bad-token", Some("user@example.com")).await;
        match result {
            Err(AppError::Auth(msg)) => assert!(
                msg.contains("email") || msg.contains("API token"),
                "Cloud 401 should mention email/API token: {}",
                msg
            ),
            other => panic!("expected AppError::Auth, got {:?}", other),
        }
    }
}
