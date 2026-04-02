use std::sync::Arc;

use axum::{extract::State, Json};

use crate::http::error::AppError;
use crate::services::initialize_service::InitializeService;
use crate::AppState;

pub async fn initialize(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let function_name = state.config.flux.function_name.clone();
    let version = env!("CARGO_PKG_VERSION");

    InitializeService::new(state)
        .run_initialization()
        .await
        .map_err(AppError::from)?;

    Ok(Json(serde_json::json!({
        "status": "initialized",
        "function": function_name,
        "version": version,
    })))
}
