use rustenium_identity::{IdentitySession, preset::random};
use rustenium::browsers::BidiBrowser;
use uuid::Uuid;

use crate::error::BrowserManagedError;

pub struct ManagedBrowser {
    id: Uuid,
    session: IdentitySession,
}

impl ManagedBrowser {
    pub async fn launch(_identity_json: Option<&str>) -> Result<Self, BrowserManagedError> {
        let identity = random();
        let mut session = IdentitySession::launch(identity).await
            .map_err(|e| BrowserManagedError::Launch(e.to_string()))?;
        session.browser_mut().connect_bidi();
        let id = Uuid::new_v4();
        tracing::info!("[ManagedBrowser] {} - Launched", id);
        Ok(Self { id, session })
    }

    pub async fn navigate(&mut self, url: &str, _wait_until: &str) -> Result<(), BrowserManagedError> {
        tracing::info!("[ManagedBrowser] {} - Navigate to: {}", self.id, url);
        self.session.browser_mut().navigate(url).await
            .map(|_| ()).map_err(|e| BrowserManagedError::Navigate(e.to_string()))
    }

    pub async fn screenshot(&self) -> Result<Vec<u8>, BrowserManagedError> {
        tracing::info!("[ManagedBrowser] {} - Screenshot", self.id);
        // let bytes = self.session.screenshot().await
        //     .map_err(|e| BrowserManagedError::Screenshot(e.to_string()))?;
        // Ok(bytes)
        Ok(Vec::new())
    }

    pub async fn click(&self, x: f32, y: f32, human: bool) -> Result<(), BrowserManagedError> {
        tracing::info!("[ManagedBrowser] {} - Click ({}, {}) human={}", self.id, x, y, human);
        Ok(())
    }

    pub async fn type_text(&self, text: &str, _selector: &str) -> Result<(), BrowserManagedError> {
        tracing::info!("[ManagedBrowser] {} - Type: {}", self.id, text);
        Ok(())
    }

    pub async fn mouse_move(&self, x: f32, y: f32, _steps: u32) -> Result<(), BrowserManagedError> {
        tracing::info!("[ManagedBrowser] {} - Mouse move ({}, {})", self.id, x, y);
        Ok(())
    }

    pub async fn human_mouse_move(&self, x: f32, y: f32) -> Result<(), BrowserManagedError> {
        tracing::info!("[ManagedBrowser] {} - Human mouse move ({}, {})", self.id, x, y);
        Ok(())
    }

    pub async fn create_context(&self, url: &str) -> Result<String, BrowserManagedError> {
        tracing::info!("[ManagedBrowser] {} - Create context url={}", self.id, url);
        Ok(Uuid::new_v4().to_string())
    }

    pub async fn close_context(&self, context_id: &str) -> Result<(), BrowserManagedError> {
        tracing::info!("[ManagedBrowser] {} - Close context {}", self.id, context_id);
        Ok(())
    }

    pub async fn eval_js(&self, script: &str) -> Result<String, BrowserManagedError> {
        tracing::info!("[ManagedBrowser] {} - Eval JS", self.id);
        let _ = script;
        Ok(String::new())
    }

    pub async fn find_node(&self, selector: &str) -> Result<bool, BrowserManagedError> {
        tracing::info!("[ManagedBrowser] {} - Find node: {}", self.id, selector);
        Ok(false)
    }

    pub async fn wait_for_node(&self, selector: &str, _timeout_ms: u64) -> Result<bool, BrowserManagedError> {
        tracing::info!("[ManagedBrowser] {} - Wait for node: {}", self.id, selector);
        Ok(false)
    }

    pub async fn emulate_device(&self, width: u32, height: u32, scale: f32, mobile: bool) -> Result<(), BrowserManagedError> {
        tracing::info!("[ManagedBrowser] {} - Emulate {}x{} scale={} mobile={}", self.id, width, height, scale, mobile);
        Ok(())
    }

    pub async fn scroll(&self, delta_x: f32, delta_y: f32) -> Result<(), BrowserManagedError> {
        tracing::info!("[ManagedBrowser] {} - Scroll dx={} dy={}", self.id, delta_x, delta_y);
        Ok(())
    }

    pub async fn close(&self) -> Result<(), BrowserManagedError> {
        tracing::info!("[ManagedBrowser] {} - Close", self.id);
        Ok(())
    }
}
