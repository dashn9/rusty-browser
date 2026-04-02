use std::sync::Arc;

use axum::{extract::Path, extract::State, http::StatusCode, Json};
use base64::Engine;
use serde::Deserialize;

use crate::http::error::AppError;
use crate::AppState;
use crate::services::browser_service::BrowserService;
use crate::services::instruct_service::AIInstructor;

fn svc(state: &Arc<AppState>) -> BrowserService {
    BrowserService::new(state.clone())
}

#[derive(Deserialize)]
pub struct CreateBrowserRequest {
    pub identity: Option<serde_json::Value>,
}

pub async fn create_browser(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateBrowserRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), AppError> {
    let browser = svc(&state)
        .create_browser(req.identity)
        .await
        .map_err(AppError::from)?;
    Ok((StatusCode::CREATED, Json(serde_json::json!({ "browser_id": browser.browser_id }))))
}

pub async fn list_browsers(State(state): State<Arc<AppState>>) -> Result<Json<serde_json::Value>, AppError> {
    let browsers = svc(&state).list_browsers().await.map_err(AppError::from)?;
    Ok(Json(serde_json::json!(browsers)))
}

pub async fn get_browser(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let browser = svc(&state)
        .get_browser(&id)
        .await
        .map_err(AppError::from)?;
    Ok(Json(serde_json::json!(browser)))
}

pub async fn delete_browser(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    svc(&state).delete_browser(&id).await.map_err(AppError::from)?;
    Ok(Json(serde_json::json!({ "deleted": id })))
}

pub async fn create_context(
    State(state): State<Arc<AppState>>,
    Path(browser_id): Path<String>,
) -> Result<(StatusCode, Json<serde_json::Value>), AppError> {
    let context_id = svc(&state)
        .create_context(&browser_id)
        .await
        .map_err(AppError::from)?;
    Ok((StatusCode::CREATED, Json(serde_json::json!({ "browser_id": browser_id, "context_id": context_id }))))
}

pub async fn delete_context(
    State(state): State<Arc<AppState>>,
    Path((browser_id, ctx_id)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, AppError> {
    svc(&state)
        .delete_context(&browser_id, &ctx_id)
        .await
        .map_err(AppError::from)?;
    Ok(Json(serde_json::json!({ "deleted_context": ctx_id, "browser_id": browser_id })))
}

#[derive(Deserialize)]
pub struct NavigateRequest {
    pub url: String,
    pub wait_until: Option<String>,
}

pub async fn navigate(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<NavigateRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    svc(&state)
        .navigate(&id, req.url, req.wait_until)
        .await
        .map_err(AppError::from)?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct ClickRequest {
    pub x: f32,
    pub y: f32,
    pub human: Option<bool>,
}

pub async fn click(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<ClickRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    svc(&state)
        .click(&id, req.x, req.y, req.human.unwrap_or(true))
        .await
        .map_err(AppError::from)?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct TypeRequest {
    pub text: String,
    pub selector: Option<String>,
}

pub async fn type_text(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<TypeRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    svc(&state)
        .type_text(&id, req.text, req.selector)
        .await
        .map_err(AppError::from)?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn screenshot(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let data = svc(&state).screenshot(&id).await.map_err(AppError::from)?
        .map(|d| base64::engine::general_purpose::STANDARD.encode(&d));
    Ok(Json(serde_json::json!({ "data": data })))
}

#[derive(Deserialize)]
pub struct EvalRequest {
    pub script: String,
}

pub async fn eval_js(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<EvalRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    svc(&state)
        .eval_js(&id, req.script)
        .await
        .map_err(AppError::from)?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct InstructRequest {
    pub instruction: String,
}

pub async fn instruct(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<InstructRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    svc(&state)
        .instruct(&id, &req.instruction)
        .await
        .map_err(AppError::from)?;
    Ok(Json(serde_json::json!({ "browser_id": id, "status": "completed" })))
}
