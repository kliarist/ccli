//! Confluence Data Center API client and PAT validation probe.
//! RED phase stub — implementation bodies pending.

use crate::api::error::AppError;
use crate::config::Config;

pub struct Client {
    inner: reqwest::Client,
    base_url: String,
    accepts_invalid_certs: bool,
}

impl Client {
    pub fn new(_config: &Config) -> Result<Self, AppError> {
        todo!("RED phase stub — implement in GREEN")
    }

    pub fn accepts_invalid_certs(&self) -> bool {
        self.accepts_invalid_certs
    }

    pub fn inner(&self) -> &reqwest::Client {
        &self.inner
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }
}

pub async fn test_connection(_base_url: &str, _token: &str) -> Result<String, AppError> {
    todo!("RED phase stub — implement in GREEN")
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::prelude::*;
    use std::sync::Mutex;

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
        std::env::set_var("CCLI_INSECURE", "true");
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
        let mut url = server.base_url();
        url.push_str("///");
        let result = test_connection(&url, "tok").await;
        mock.assert();
        assert_eq!(result.unwrap(), "Bob");
    }

    #[tokio::test]
    async fn test_connection_returns_network_error_on_unreachable_host() {
        let result = test_connection("http://127.0.0.1:1", "tok").await;
        match result {
            Err(AppError::Network(_)) => {}
            other => panic!("expected AppError::Network, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_connection_returns_auth_error_on_200_with_empty_body() {
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
