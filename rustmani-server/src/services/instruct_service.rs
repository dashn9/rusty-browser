use async_trait::async_trait;
use base64::Engine;
use std::sync::Arc;

use rustmani_common::ai::BrowserAction;
use rustmani_common::error::RustmaniError;

use crate::AppState;

#[async_trait]
pub trait AIInstructor: Send + Sync {
    fn state(&self) -> &Arc<AppState>;
    
    async fn screenshot(&self, browser_id: &str) -> Result<Option<Vec<u8>>, RustmaniError>;
    
    async fn dispatch(&self, browser_id: &str, action: &BrowserAction) -> Result<(), RustmaniError>;
    
    async fn instruct(&self, browser_id: &str, instruction: &str) -> Result<(), RustmaniError> {
        let raw = self.screenshot(browser_id).await?
            .ok_or_else(|| RustmaniError::Internal("No screenshot data".into()))?;

        let processed = downscale(
            &raw,
            self.state().config.ai.resolution.max_width,
            self.state().config.ai.resolution.quality,
        )?;
        let b64 = base64::engine::general_purpose::STANDARD.encode(&processed);

        let ai_response = self.state().ai_provider
            .analyze_screenshot(b64, instruction.to_string())
            .await
            .map_err(|e| RustmaniError::Internal(e.to_string()))?;

        for action in &ai_response {
            self.dispatch(browser_id, action).await?;
        }

        Ok(())
    }
}

fn downscale(data: &[u8], max_width: u32, quality: u32) -> Result<Vec<u8>, RustmaniError> {
    if data.is_empty() {
        return Ok(data.to_vec());
    }
    let img = image::load_from_memory(data)
        .map_err(|e| RustmaniError::Internal(format!("Decode: {e}")))?;
    let img = if img.width() > max_width {
        let ratio = max_width as f32 / img.width() as f32;
        img.resize(
            max_width,
            (img.height() as f32 * ratio) as u32,
            image::imageops::FilterType::Lanczos3,
        )
    } else {
        img
    };
    let mut buf = std::io::Cursor::new(Vec::new());
    img.write_with_encoder(image::codecs::jpeg::JpegEncoder::new_with_quality(
        &mut buf,
        quality as u8,
    ))
    .map_err(|e| RustmaniError::Internal(format!("Encode: {e}")))?;
    Ok(buf.into_inner())
}
