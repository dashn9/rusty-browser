use thiserror::Error;

/// Errors scoped to browser command dispatch and gRPC connectivity (server-side).
#[derive(Debug, Error)]
pub enum BrowserError {
    #[error("Browser not found: {0}")]
    NotFound(String),

    #[error("gRPC connection failed: {0}")]
    Connect(String),

    #[error("gRPC command failed: {0}")]
    Command(String),

    #[error("Screenshot returned no data")]
    NoScreenshot,
}

impl From<BrowserError> for RustmaniError {
    fn from(e: BrowserError) -> Self {
        match e {
            BrowserError::NotFound(id) => RustmaniError::BrowserNotFound(id),
            other => RustmaniError::Internal(other.to_string()),
        }
    }
}

#[derive(Debug, Error)]
pub enum FluxError {
    #[error("Flux HTTP error ({status}): {body}")]
    Http { status: u16, body: String },

    #[error("Flux execution error: {0}")]
    Execution(String),

    #[error("Failed to parse Flux response: {0}")]
    Parse(String),

    #[error("Flux request failed: {0}")]
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
pub enum RustmaniError {
    #[error("Redis error: {0}")]
    Redis(#[from] redis::RedisError),

    #[error("Browser not found: {0}")]
    BrowserNotFound(String),

    #[error("Flux error: {0}")]
    Flux(#[from] FluxError),

    #[error("AI provider error: {0}")]
    AIProvider(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Unauthorized")]
    Unauthorized,

    #[error("{0}")]
    Internal(String),
}
