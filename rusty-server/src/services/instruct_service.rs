use base64::Engine;
use std::sync::Arc;

use rusty_common::ai::BrowserAction;

use crate::http::error::AppError;
use crate::AppState;

pub trait AIInstructor: Send + Sync {
    fn state(&self) -> &Arc<AppState>;

    async fn screenshot(&self, execution_id: &str) -> Result<Option<Vec<u8>>, AppError>;

    async fn dispatch(&self, execution_id: &str, action: &BrowserAction) -> Result<(), AppError>;

    async fn instruct(&self, execution_id: &str, instruction: &str) -> Result<(), AppError> {
        let raw = self.screenshot(execution_id).await?
            .ok_or_else(|| AppError::Internal("No screenshot data".into()))?;

        let b64 = base64::engine::general_purpose::STANDARD.encode(&raw);

        let ai_response = self.state().ai_provider
            .analyze_screenshot(b64, instruction.to_string())
            .await
            .map_err(AppError::AI)?;

        for action in &ai_response {
            self.dispatch(execution_id, action).await?;
        }

        Ok(())
    }
}

