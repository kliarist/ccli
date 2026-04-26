// Stub — implementation will be added in GREEN phase
#[derive(Debug, Clone)]
pub enum AppError {
    Auth(String),
    Network(String),
    Config(String),
    Api(String),
}

// NOTE: std::error::Error and Display not yet implemented — tests will fail

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
