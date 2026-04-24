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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_error_read_display() {
        let e = ConfigError::Read("no such file".to_string());
        assert!(e.to_string().contains("no such file"));
    }

    #[test]
    fn config_error_parse_display() {
        let e = ConfigError::Parse("unexpected token".to_string());
        assert!(e.to_string().contains("unexpected token"));
    }

    #[test]
    fn flux_error_http_display() {
        let e = FluxError::Http { status: 404, body: "not found".to_string() };
        let s = e.to_string();
        assert!(s.contains("404"));
        assert!(s.contains("not found"));
    }

    #[test]
    fn flux_error_execution_display() {
        let e = FluxError::Execution("timeout".to_string());
        assert!(e.to_string().contains("timeout"));
    }

    #[test]
    fn flux_error_parse_display() {
        let e = FluxError::Parse("invalid json".to_string());
        assert!(e.to_string().contains("invalid json"));
    }

    #[test]
    fn ai_error_request_failed_display() {
        let e = AIError::RequestFailed("network error".to_string());
        assert!(e.to_string().contains("network error"));
    }

    #[test]
    fn ai_error_invalid_response_display() {
        let e = AIError::InvalidResponse("bad args for navigate".to_string());
        assert!(e.to_string().contains("bad args for navigate"));
    }

    #[test]
    fn ai_error_unauthorized_display() {
        let e = AIError::Unauthorized;
        assert!(e.to_string().contains("unauthorized"));
    }

    #[test]
    fn ai_error_rate_limited_display() {
        let e = AIError::RateLimited;
        assert!(e.to_string().contains("rate limited"));
    }

    #[test]
    fn grpc_error_connect_display() {
        let e = GrpcError::Connect("refused".to_string());
        assert!(e.to_string().contains("refused"));
    }

    #[test]
    fn grpc_error_command_display() {
        let e = GrpcError::Command("execute failed".to_string());
        assert!(e.to_string().contains("execute failed"));
    }

    #[test]
    fn browser_error_not_found_display() {
        let e = BrowserError::NotFound("exec-123".to_string());
        let s = e.to_string();
        assert!(s.contains("exec-123"));
    }

    #[test]
    fn browser_error_no_screenshot_display() {
        let e = BrowserError::NoScreenshot;
        assert!(e.to_string().contains("screenshot"));
    }
}
