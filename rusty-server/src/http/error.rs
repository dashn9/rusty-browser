use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

use rusty_common::error::{AIError, BrowserError, FluxError, GrpcError, StorageError};

#[derive(Serialize)]
struct ErrorResponse {
    error: ErrorDetail,
}

#[derive(Serialize)]
struct ErrorDetail {
    code: String,
    message: String,
}

pub enum AppError {
    Storage(StorageError),
    Flux(FluxError),
    AI(AIError),
    Grpc(GrpcError),
    Browser(BrowserError),
    Internal(String),
    NotFound(String),
    Unauthorized,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            AppError::Storage(e) => {
                tracing::error!("Storage error: {e}");
                (StatusCode::INTERNAL_SERVER_ERROR, "STORAGE_ERROR", e.to_string())
            }
            AppError::Flux(e) => {
                tracing::error!("Flux error: {e}");
                (StatusCode::BAD_GATEWAY, "FLUX_ERROR", e.to_string())
            }
            AppError::AI(e) => {
                tracing::error!("AI error: {e}");
                (StatusCode::BAD_GATEWAY, "AI_ERROR", e.to_string())
            }
            AppError::Grpc(e) => {
                tracing::error!("gRPC error: {e}");
                (StatusCode::BAD_GATEWAY, "GRPC_ERROR", e.to_string())
            }
            AppError::Browser(BrowserError::NotFound(id)) => {
                (StatusCode::NOT_FOUND, "NOT_FOUND", format!("Browser not found: {id}"))
            }
            AppError::Browser(e) => {
                (StatusCode::BAD_GATEWAY, "BROWSER_ERROR", e.to_string())
            }
            AppError::Internal(msg) => {
                tracing::error!("Internal error: {msg}");
                (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR", msg)
            }
            AppError::NotFound(id) => {
                (StatusCode::NOT_FOUND, "NOT_FOUND", format!("Not found: {id}"))
            }
            AppError::Unauthorized => {
                (StatusCode::UNAUTHORIZED, "UNAUTHORIZED", "Unauthorized".to_string())
            }
        };

        let body = ErrorResponse {
            error: ErrorDetail { code: code.to_string(), message },
        };
        (status, Json(body)).into_response()
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::Storage(e) => write!(f, "storage error: {e}"),
            AppError::Flux(e) => write!(f, "flux error: {e}"),
            AppError::AI(e) => write!(f, "AI error: {e}"),
            AppError::Grpc(e) => write!(f, "gRPC error: {e}"),
            AppError::Browser(e) => write!(f, "browser error: {e}"),
            AppError::Internal(msg) => write!(f, "internal error: {msg}"),
            AppError::NotFound(id) => write!(f, "not found: {id}"),
            AppError::Unauthorized => write!(f, "unauthorized"),
        }
    }
}

impl From<StorageError> for AppError {
    fn from(e: StorageError) -> Self { AppError::Storage(e) }
}

impl From<FluxError> for AppError {
    fn from(e: FluxError) -> Self { AppError::Flux(e) }
}

impl From<AIError> for AppError {
    fn from(e: AIError) -> Self { AppError::AI(e) }
}

impl From<GrpcError> for AppError {
    fn from(e: GrpcError) -> Self { AppError::Grpc(e) }
}

impl From<BrowserError> for AppError {
    fn from(e: BrowserError) -> Self { AppError::Browser(e) }
}
