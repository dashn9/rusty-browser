use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

use rustmani_common::error::{FluxError, RustmaniError};

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
    Flux(FluxError),
    Internal(String),
    NotFound(String),
    Unauthorized,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            AppError::Flux(e) => {
                let msg = e.to_string();
                tracing::error!("Flux error: {msg}");
                (StatusCode::BAD_GATEWAY, "FLUX_ERROR".to_string(), msg)
            }
            AppError::Internal(msg) => {
                tracing::error!("Internal error: {msg}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "INTERNAL_ERROR".to_string(),
                    msg,
                )
            }
            AppError::NotFound(id) => {
                let msg = format!("Resource not found: {id}");
                (StatusCode::NOT_FOUND, "NOT_FOUND".to_string(), msg)
            }
            AppError::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "UNAUTHORIZED".to_string(),
                "Unauthorized".to_string(),
            ),
        };

        let body = ErrorResponse {
            error: ErrorDetail { code, message },
        };
        (status, Json(body)).into_response()
    }
}

impl From<RustmaniError> for AppError {
    fn from(e: RustmaniError) -> Self {
        match e {
            RustmaniError::Flux(f) => AppError::Flux(f),
            RustmaniError::BrowserNotFound(id) => AppError::NotFound(id),
            RustmaniError::Unauthorized => AppError::Unauthorized,
            RustmaniError::Internal(msg) => AppError::Internal(msg),
            other => AppError::Internal(other.to_string()),
        }
    }
}

impl From<FluxError> for AppError {
    fn from(e: FluxError) -> Self {
        AppError::Flux(e)
    }
}
