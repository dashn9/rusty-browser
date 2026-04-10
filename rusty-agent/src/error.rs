use thiserror::Error;

#[derive(Debug, Error)]
pub enum TlsError {
    #[error("cert read failed: {0}")]
    CertRead(String),
    #[error("key read failed: {0}")]
    KeyRead(String),
    #[error("config failed: {0}")]
    Config(String),
}

#[derive(Debug, Error)]
pub enum GrpcError {
    #[error("serve failed: {0}")]
    Serve(String),
}

#[derive(Debug, Error)]
pub enum BrowserError {
    #[error("launch failed: {0}")]
    Launch(String),
    #[error("navigate failed: {0}")]
    Navigate(String),
    #[error("screenshot failed: {0}")]
    Screenshot(String),
    #[error("click failed: {0}")]
    Click(String),
    #[error("type text failed: {0}")]
    TypeText(String),
    #[error("eval JS failed: {0}")]
    EvalJs(String),
    #[error("context operation failed: {0}")]
    Context(String),
    #[error("close failed: {0}")]
    Close(String),
    #[error("action failed: {0}")]
    Action(String),
}

#[derive(Debug, Error)]
pub enum AgentError {
    #[error("TLS error: {0}")]
    Tls(#[from] TlsError),
    #[error("gRPC error: {0}")]
    Grpc(#[from] GrpcError),
    #[error("Browser error: {0}")]
    Browser(#[from] BrowserError),
}
