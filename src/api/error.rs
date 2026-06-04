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

pub fn map_network_error(e: reqwest::Error) -> AppError {
    if e.is_connect() || e.is_timeout() {
        AppError::Network(format!("Cannot reach server: {}", e))
    } else {
        AppError::Network(e.to_string())
    }
}
