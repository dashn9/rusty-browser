use std::sync::Arc;

use uuid::Uuid;

use rustmani_common::ai::BrowserAction;
use rustmani_common::error::RustmaniError;
use rustmani_common::state::BrowserInfo;

use crate::AppState;

pub struct BrowserService {
    pub state: Arc<AppState>,
}

impl BrowserService {
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }

    pub async fn create_browser(&self, identity: Option<serde_json::Value>) -> Result<BrowserInfo, RustmaniError> {
        let mut args = vec![];
        if let Some(id) = identity {
            args.push(id.to_string());
        }
        let browser = self.state.flux.execute_function(&self.state.config.flux.function_name, &args).await?;
        self.state.redis.add_browser(&browser).await?;
        Ok(browser)
    }

    pub async fn list_browsers(&self) -> Result<Vec<BrowserInfo>, RustmaniError> {
        self.state.redis.list_browsers().await
    }

    pub async fn get_browser(&self, id: &str) -> Result<BrowserInfo, RustmaniError> {
        self.state.redis.get_browser(id).await
    }

    pub async fn delete_browser(&self, id: &str) -> Result<(), RustmaniError> {
        let browser = self.state.redis.get_browser(id).await?;
        self.connect(&browser).await?
            .execute(tonic::Request::new(self.cmd(id, "",
                rustmani_proto::browser_command::Action::CloseBrowser(rustmani_proto::CloseBrowser {}),
            )))
            .await
            .map_err(|e| RustmaniError::Internal(format!("CloseBrowser: {e}")))?;
        self.state.redis.remove_browser(id).await
    }

    pub async fn create_context(&self, browser_id: &str) -> Result<String, RustmaniError> {
        let browser = self.state.redis.get_browser(browser_id).await?;
        let context_id = Uuid::new_v4().to_string();
        self.connect(&browser).await?
            .execute(tonic::Request::new(self.cmd(browser_id, &context_id,
                rustmani_proto::browser_command::Action::CreateContext(
                    rustmani_proto::CreateContext { url: None },
                ),
            )))
            .await
            .map_err(|e| RustmaniError::Internal(format!("CreateContext: {e}")))?;
        self.state.redis.add_context(browser_id, &context_id).await?;
        Ok(context_id)
    }

    pub async fn delete_context(&self, browser_id: &str, ctx_id: &str) -> Result<(), RustmaniError> {
        let browser = self.state.redis.get_browser(browser_id).await?;
        self.connect(&browser).await?
            .execute(tonic::Request::new(self.cmd(browser_id, ctx_id,
                rustmani_proto::browser_command::Action::CloseContext(
                    rustmani_proto::CloseContext { context_id: ctx_id.to_string() },
                ),
            )))
            .await
            .map_err(|e| RustmaniError::Internal(format!("CloseContext: {e}")))?;
        self.state.redis.remove_context(browser_id, ctx_id).await
    }

    pub async fn navigate(&self, browser_id: &str, url: String, wait_until: Option<String>) -> Result<(), RustmaniError> {
        let browser = self.state.redis.get_browser(browser_id).await?;
        self.connect(&browser).await?
            .execute(tonic::Request::new(self.cmd(browser_id, "",
                rustmani_proto::browser_command::Action::Navigate(
                    rustmani_proto::Navigate { url, wait_until: wait_until.unwrap_or_default() },
                ),
            )))
            .await
            .map_err(|e| RustmaniError::Internal(format!("Navigate: {e}")))?;
        Ok(())
    }

    pub async fn click(&self, browser_id: &str, x: f32, y: f32, human: bool) -> Result<(), RustmaniError> {
        let browser = self.state.redis.get_browser(browser_id).await?;
        self.connect(&browser).await?
            .execute(tonic::Request::new(self.cmd(browser_id, "",
                rustmani_proto::browser_command::Action::Click(
                    rustmani_proto::Click { selector: None, x: Some(x), y: Some(y), human },
                ),
            )))
            .await
            .map_err(|e| RustmaniError::Internal(format!("Click: {e}")))?;
        Ok(())
    }

    pub async fn type_text(&self, browser_id: &str, text: String, selector: Option<String>) -> Result<(), RustmaniError> {
        let browser = self.state.redis.get_browser(browser_id).await?;
        self.connect(&browser).await?
            .execute(tonic::Request::new(self.cmd(browser_id, "",
                rustmani_proto::browser_command::Action::TypeText(
                    rustmani_proto::Type { text, selector },
                ),
            )))
            .await
            .map_err(|e| RustmaniError::Internal(format!("TypeText: {e}")))?;
        Ok(())
    }

    pub async fn screenshot(&self, browser_id: &str) -> Result<Option<Vec<u8>>, RustmaniError> {
        let browser = self.state.redis.get_browser(browser_id).await?;
        let result = self.connect(&browser).await?
            .execute(tonic::Request::new(self.cmd(browser_id, "",
                rustmani_proto::browser_command::Action::Screenshot(
                    rustmani_proto::Screenshot {
                        quality: self.state.config.ai.resolution.quality,
                        format: self.state.config.ai.resolution.format.clone(),
                    },
                ),
            )))
            .await
            .map_err(|e| RustmaniError::Internal(format!("Screenshot: {e}")))?
            .into_inner();
        Ok(result.screenshot.map(|s| s.data))
    }

    pub async fn eval_js(&self, browser_id: &str, script: String) -> Result<(), RustmaniError> {
        let browser = self.state.redis.get_browser(browser_id).await?;
        self.connect(&browser).await?
            .execute(tonic::Request::new(self.cmd(browser_id, "",
                rustmani_proto::browser_command::Action::EvalJs(rustmani_proto::EvalJs { script }),
            )))
            .await
            .map_err(|e| RustmaniError::Internal(format!("EvalJs: {e}")))?;
        Ok(())
    }

    pub async fn dispatch(&self, browser_id: &str, action: &BrowserAction) -> Result<(), RustmaniError> {
        let browser = self.state.redis.get_browser(browser_id).await?;
        let mut client = self.connect(&browser).await?;

        let proto_action = match action {
            BrowserAction::Navigate { url } => rustmani_proto::browser_command::Action::Navigate(
                rustmani_proto::Navigate { url: url.clone(), wait_until: "complete".to_string() },
            ),
            BrowserAction::Click { x, y, human } => rustmani_proto::browser_command::Action::Click(
                rustmani_proto::Click { selector: None, x: Some(*x), y: Some(*y), human: *human },
            ),
            BrowserAction::Type { text, selector } => rustmani_proto::browser_command::Action::TypeText(
                rustmani_proto::Type { text: text.clone(), selector: selector.clone() },
            ),
            BrowserAction::MouseMove { x, y } => rustmani_proto::browser_command::Action::HumanMouseMove(
                rustmani_proto::HumanMouseMove { selector: None, x: Some(*x), y: Some(*y) },
            ),
            BrowserAction::Scroll { delta_x, delta_y } => rustmani_proto::browser_command::Action::Scroll(
                rustmani_proto::Scroll { delta_x: *delta_x, delta_y: *delta_y },
            ),
            BrowserAction::Wait { ms } => {
                tokio::time::sleep(std::time::Duration::from_millis(*ms)).await;
                return Ok(());
            }
            BrowserAction::Screenshot | BrowserAction::Done { .. } => return Ok(()),
        };

        client.execute(tonic::Request::new(self.cmd(browser_id, "", proto_action)))
            .await
            .map_err(|e| RustmaniError::Internal(format!("Dispatch: {e}")))?;
        Ok(())
    }

    async fn connect(&self, browser: &BrowserInfo) -> Result<rustmani_proto::browser_agent_client::BrowserAgentClient<tonic::transport::Channel>, RustmaniError> {
        let addr = format!("https://{}:{}", browser.host, browser.grpc_port);
        rustmani_proto::browser_agent_client::BrowserAgentClient::connect(addr)
            .await
            .map_err(|e| RustmaniError::Internal(format!("Connect: {e}")))
    }

    fn cmd(&self, browser_id: &str, context_id: &str, action: rustmani_proto::browser_command::Action) -> rustmani_proto::BrowserCommand {
        rustmani_proto::BrowserCommand {
            browser_id: browser_id.to_string(),
            context_id: context_id.to_string(),
            action: Some(action),
        }
    }
}
