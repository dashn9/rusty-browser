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

#[cfg(test)]
mod tests {
    use super::*;

    // ---- TlsError ----

    #[test]
    fn tls_error_cert_read_display() {
        let e = TlsError::CertRead("no cert".to_string());
        assert!(e.to_string().contains("no cert"));
    }

    #[test]
    fn tls_error_key_read_display() {
        let e = TlsError::KeyRead("no key".to_string());
        assert!(e.to_string().contains("no key"));
    }

    #[test]
    fn tls_error_config_display() {
        let e = TlsError::Config("bad config".to_string());
        assert!(e.to_string().contains("bad config"));
    }

    // ---- GrpcError ----

    #[test]
    fn grpc_error_serve_display() {
        let e = GrpcError::Serve("port in use".to_string());
        assert!(e.to_string().contains("port in use"));
    }

    // ---- BrowserError ----

    #[test]
    fn browser_error_launch_display() {
        let e = BrowserError::Launch("chromedriver not found".to_string());
        assert!(e.to_string().contains("chromedriver not found"));
    }

    #[test]
    fn browser_error_navigate_display() {
        let e = BrowserError::Navigate("net::ERR_NAME_NOT_RESOLVED".to_string());
        assert!(e.to_string().contains("net::ERR_NAME_NOT_RESOLVED"));
    }

    #[test]
    fn browser_error_screenshot_display() {
        let e = BrowserError::Screenshot("page not loaded".to_string());
        assert!(e.to_string().contains("page not loaded"));
    }

    #[test]
    fn browser_error_click_display() {
        let e = BrowserError::Click("element not clickable".to_string());
        assert!(e.to_string().contains("element not clickable"));
    }

    #[test]
    fn browser_error_type_text_display() {
        let e = BrowserError::TypeText("focus lost".to_string());
        assert!(e.to_string().contains("focus lost"));
    }

    #[test]
    fn browser_error_eval_js_display() {
        let e = BrowserError::EvalJs("syntax error".to_string());
        assert!(e.to_string().contains("syntax error"));
    }

    #[test]
    fn browser_error_context_display() {
        let e = BrowserError::Context("tab closed".to_string());
        assert!(e.to_string().contains("tab closed"));
    }

    #[test]
    fn browser_error_close_display() {
        let e = BrowserError::Close("session gone".to_string());
        assert!(e.to_string().contains("session gone"));
    }

    #[test]
    fn browser_error_action_display() {
        let e = BrowserError::Action("scroll failed".to_string());
        assert!(e.to_string().contains("scroll failed"));
    }

    // ---- AgentError conversions ----

    #[test]
    fn agent_error_from_tls_display() {
        let e: AgentError = TlsError::CertRead("x".to_string()).into();
        assert!(e.to_string().contains("TLS"));
    }

    #[test]
    fn agent_error_from_grpc_display() {
        let e: AgentError = GrpcError::Serve("y".to_string()).into();
        assert!(e.to_string().contains("gRPC"));
    }

    #[test]
    fn agent_error_from_browser_display() {
        let e: AgentError = BrowserError::Launch("z".to_string()).into();
        assert!(e.to_string().contains("Browser"));
    }
}
