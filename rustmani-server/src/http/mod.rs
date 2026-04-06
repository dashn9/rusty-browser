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
        .route("/browsers/{execution_id}/", get(browsers::get_browser))
        .route("/browsers/{execution_id}/", delete(browsers::delete_browser))
        .route("/browsers/{execution_id}/contexts/", put(browsers::create_context))
        .route("/browsers/{execution_id}/contexts/{ctx_id}/", delete(browsers::delete_context))
        .route("/browsers/{execution_id}/navigate/", post(browsers::navigate))
        .route("/browsers/{execution_id}/click/", post(browsers::click))
        .route("/browsers/{execution_id}/type/", post(browsers::type_text))
        .route("/browsers/{execution_id}/screenshot/", post(browsers::screenshot))
        .route("/browsers/{execution_id}/eval/", post(browsers::eval_js))
        .route("/browsers/{execution_id}/scroll-by/", post(browsers::scroll_by))
        .route("/browsers/{execution_id}/scroll-to/", post(browsers::scroll_to))
        .route("/browsers/{execution_id}/instruct/", post(browsers::instruct))
        .route("/browsers/{execution_id}/node-click/", post(browsers::node_click))
        .route("/browsers/{execution_id}/fetch-html/", post(browsers::fetch_html))
        .route("/browsers/{execution_id}/fetch-text/", post(browsers::fetch_text))
        .route("/browsers/{execution_id}/logs/", get(browsers::get_execution_logs))
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
