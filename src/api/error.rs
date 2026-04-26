use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum AppError {
    #[error("Authentication failed: {0}")]
    Auth(String),

    #[error("Network error: {0}")]
    Network(String),

    #[error("Config error: {0}")]
    Config(String),

    #[error("API error: {0}")]
    Api(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_display() {
        assert_eq!(AppError::Auth("bad token".into()).to_string(), "Authentication failed: bad token");
    }

    #[test]
    fn network_display() {
        assert_eq!(AppError::Network("timeout".into()).to_string(), "Network error: timeout");
    }

    #[test]
    fn config_display() {
        assert_eq!(AppError::Config("missing".into()).to_string(), "Config error: missing");
    }

    #[test]
    fn api_display() {
        assert_eq!(AppError::Api("HTTP 500".into()).to_string(), "API error: HTTP 500");
    }

    #[test]
    fn implements_std_error() {
        fn assert_err<E: std::error::Error>(_: E) {}
        assert_err(AppError::Auth("x".into()));
    }

    #[test]
    fn implements_clone() {
        let a = AppError::Auth("bad token".into());
        let b = a.clone();
        assert_eq!(a.to_string(), b.to_string());
    }
}
