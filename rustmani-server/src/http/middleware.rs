use std::sync::Arc;

use axum::{
    extract::State,
    http::Request,
    middleware::Next,
    response::Response,
};

use crate::http::error::AppError;
use crate::AppState;

pub async fn request_logger(request: Request<axum::body::Body>, next: Next) -> Response {
    tracing::info!("{} {}", request.method(), request.uri());
    next.run(request).await
}

pub async fn api_key_auth(
    State(state): State<Arc<AppState>>,
    request: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, AppError> {
    let api_key = request
        .headers()
        .get("X-API-Key")
        .or_else(|| request.headers().get("authorization"))
        .and_then(|v| v.to_str().ok())
        .map(|s| s.strip_prefix("Bearer ").unwrap_or(s));

    match api_key {
        Some(key) if state.config.api_keys.contains(&key.to_string()) => {
            Ok(next.run(request).await)
        }
        _ => Err(AppError::Unauthorized),
    }
}
