use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;

use crate::services::initialize_service::InitializeService;
use crate::AppState;

/// `POST /initialize`
///
/// Triggers the full agent deployment pipeline:
///   1. Initialize Flux
///   2. Register the `rustmani-agent` function
///   3. Download the matching agent `.deb` from GitHub Releases
///   4. Upload to Flux via multipart/form-data
pub async fn initialize(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let function_name = state.config.flux.function_name.clone();
    let version = env!("CARGO_PKG_VERSION");

    InitializeService::new(state)
        .run_initialization()
        .await
        .map_err(|e| {
            tracing::error!("Initialization failed: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(serde_json::json!({
        "status": "initialized",
        "function": function_name,
        "version": version,
    })))
}
