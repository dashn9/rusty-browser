pub mod error;
pub mod handlers;
pub mod middleware;

use std::sync::Arc;

use axum::{routing::{delete, get, post, put}, Router};

use crate::AppState;
use handlers::{browsers, initialize};

pub fn router(state: Arc<AppState>) -> Router {
    let public = Router::new()
        .route("/health", get(|| async { "ok" }));

    let protected = Router::new()
        .route("/initialize/", post(initialize::initialize))
        .route("/browsers/", put(browsers::create_browser))
        .route("/browsers/", get(browsers::list_browsers))
        .route("/browsers/{id}/", get(browsers::get_browser))
        .route("/browsers/{id}/", delete(browsers::delete_browser))
        .route("/browsers/{id}/contexts/", put(browsers::create_context))
        .route("/browsers/{browser_id}/contexts/{ctx_id}/", delete(browsers::delete_context))
        .route("/browsers/{id}/navigate/", post(browsers::navigate))
        .route("/browsers/{id}/click/", post(browsers::click))
        .route("/browsers/{id}/type/", post(browsers::type_text))
        .route("/browsers/{id}/screenshot/", post(browsers::screenshot))
        .route("/browsers/{id}/eval/", post(browsers::eval_js))
        .route("/browsers/{id}/instruct/", post(browsers::instruct))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::api_key_auth,
        ));

    Router::new()
        .merge(public)
        .merge(protected)
        .layer(axum::middleware::from_fn(middleware::request_logger))
        .with_state(state)
}
