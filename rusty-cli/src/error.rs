use thiserror::Error;

#[derive(Debug, Error)]
pub enum CliError {
    #[error("HTTP request failed: {0}")]
    Http(String),

    #[error("Response parse error: {0}")]
    Parse(String),

    #[error("Server returned {status}: {body}")]
    ServerError { status: u16, body: String },
}
