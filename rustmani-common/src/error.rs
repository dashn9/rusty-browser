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

#[derive(Debug)]
pub enum AIError {
    RequestFailed(String),
    InvalidResponse(String),
    Unauthorized,
    RateLimited,
}

impl std::fmt::Display for AIError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AIError::RequestFailed(msg) => write!(f, "request failed: {}", msg),
            AIError::InvalidResponse(msg) => write!(f, "invalid response: {}", msg),
            AIError::Unauthorized => write!(f, "unauthorized"),
            AIError::RateLimited => write!(f, "rate limited"),
        }
    }
}

impl std::error::Error for AIError {}

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
