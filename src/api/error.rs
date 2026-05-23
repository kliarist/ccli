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
