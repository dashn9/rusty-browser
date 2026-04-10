use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("redis error: {0}")]
    Redis(#[from] redis::RedisError),
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read config file: {0}")]
    Read(String),
    #[error("failed to parse config: {0}")]
    Parse(String),
}

#[derive(Debug, Error)]
pub enum FluxError {
    #[error("HTTP error ({status}): {body}")]
    Http { status: u16, body: String },

    #[error("execution error: {0}")]
    Execution(String),

    #[error("failed to parse response: {0}")]
    Parse(String),

    #[error("request failed: {0}")]
    Request(#[from] reqwest::Error),
}

#[derive(Debug, Error)]
pub enum AIError {
    #[error("request failed: {0}")]
    RequestFailed(String),
    #[error("invalid response: {0}")]
    InvalidResponse(String),
    #[error("unauthorized")]
    Unauthorized,
    #[error("rate limited")]
    RateLimited,
}

#[derive(Debug, Error)]
pub enum GrpcError {
    #[error("connection failed: {0}")]
    Connect(String),
    #[error("command failed: {0}")]
    Command(String),
}

#[derive(Debug, Error)]
pub enum BrowserError {
    #[error("browser not found: {0}")]
    NotFound(String),
    #[error("no screenshot data")]
    NoScreenshot,
}
