use thiserror::Error;

#[derive(Debug, Error)]
pub enum AgentError {
    #[error("Browser launch failed: {0}")]
    BrowserLaunch(String),

    #[error("Command execution failed: {0}")]
    Execution(String),

    #[error("gRPC server error: {0}")]
    GrpcServe(String),

    #[error("TLS configuration error: {0}")]
    Tls(String),
}

/// Errors scoped to managed browser operations (launch, navigate, screenshot, click, etc.)
#[derive(Debug, Error)]
pub enum BrowserManagedError {
    #[error("Launch failed: {0}")]
    Launch(String),

    #[error("Navigate failed: {0}")]
    Navigate(String),

    #[error("Screenshot failed: {0}")]
    Screenshot(String),

    #[error("Click failed: {0}")]
    Click(String),

    #[error("Type text failed: {0}")]
    TypeText(String),

    #[error("Eval JS failed: {0}")]
    EvalJs(String),

    #[error("Context operation failed: {0}")]
    Context(String),

    #[error("Browser close failed: {0}")]
    Close(String),
}

impl From<BrowserManagedError> for anyhow::Error {
    fn from(e: BrowserManagedError) -> Self {
        anyhow::anyhow!("{e}")
    }
}
