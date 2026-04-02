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
        let (status, code, message) = match &self {
            AppError::Flux(e) => {
                let msg = e.to_string();
                (StatusCode::BAD_GATEWAY, "FLUX_ERROR", msg)
            }
            AppError::Internal(msg) => {
                tracing::error!("Internal error: {msg}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "INTERNAL_ERROR",
                    msg.clone(),
                )
            }
            AppError::NotFound(id) => {
                let msg = format!("Resource not found: {id}");
                (StatusCode::NOT_FOUND, "NOT_FOUND", msg)
            }
            AppError::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "UNAUTHORIZED",
                "Unauthorized".to_string(),
            ),
        };

        let body = ErrorResponse {
            error: ErrorDetail {
                code: code.to_string(),
                message,
            },
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
