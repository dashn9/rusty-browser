use std::sync::Arc;

use async_trait::async_trait;
use uuid::Uuid;

use rustmani_common::ai::BrowserAction;
use rustmani_common::error::BrowserError;
use rustmani_common::state::BrowserInfo;
use rustmani_proto::browser_agent_client::BrowserAgentClient;
use rustmani_proto::browser_command::Action;
use rustmani_proto::*;

use crate::http::error::AppError;
use crate::services::instruct_service::AIInstructor;
use crate::AppState;

type GrpcClient = BrowserAgentClient<tonic::transport::Channel>;

pub struct BrowserService {
    pub state: Arc<AppState>,
}

impl BrowserService {
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }

    /// Spawns a new agent via Flux. Returns the execution_id — the agent registers back
    /// via gRPC with its browser_id and connection details.
    pub async fn create_browser(&self, identity: Option<serde_json::Value>) -> Result<String, AppError> {
        let master_url = format!(
            "https://{}:{}",
            self.state.public_ip,
            self.state.config.server.grpc_port.expect("grpc_port resolved at startup"),
        );
        // master_url is argv[1]; any identity args follow
        let mut args = vec![master_url.clone()];
        args.extend(identity.into_iter().map(|v| v.to_string()));
        let execution_id = self.state.flux.spawn_agent(&self.state.config.flux.function_name, &args).await?;
        self.state.redis.store_pending_execution(&execution_id).await?;
        Ok(execution_id)
    }

    pub async fn list_browsers(&self) -> Result<Vec<BrowserInfo>, AppError> {
        Ok(self.state.redis.list_browsers().await?)
    }

    pub async fn get_browser(&self, execution_id: &str) -> Result<BrowserInfo, AppError> {
        self.state.redis.get_browser(execution_id).await?
            .ok_or_else(|| AppError::Browser(BrowserError::NotFound(execution_id.to_string())))
    }

    pub async fn delete_browser(&self, execution_id: &str) -> Result<(), AppError> {
        self.exec(execution_id, "", Action::CloseBrowser(CloseBrowser {})).await
            .or_else(|e| match e {
                AppError::Browser(BrowserError::NotFound(_)) => Ok(()),
                other => Err(other),
            })?;
        self.state.redis.remove_browser(execution_id).await?;
        Ok(())
    }

    pub async fn create_context(&self, execution_id: &str) -> Result<String, AppError> {
        let context_id = Uuid::new_v4().to_string();
        self.exec(execution_id, &context_id, Action::CreateContext(CreateContext { url: None })).await?;
        self.state.redis.add_context(execution_id, &context_id).await?;
        Ok(context_id)
    }

    pub async fn delete_context(&self, execution_id: &str, ctx_id: &str) -> Result<(), AppError> {
        self.exec(execution_id, ctx_id, Action::CloseContext(CloseContext { context_id: ctx_id.to_string() })).await?;
        self.state.redis.remove_context(execution_id, ctx_id).await?;
        Ok(())
    }

    pub async fn navigate(&self, execution_id: &str, url: String, wait_until: Option<String>) -> Result<(), AppError> {
        self.exec(execution_id, "", Action::Navigate(Navigate { url, wait_until: wait_until.unwrap_or_default() })).await
    }

    pub async fn click(&self, execution_id: &str, x: f32, y: f32, human: bool) -> Result<(), AppError> {
        self.exec(execution_id, "", Action::Click(Click { selector: None, x: Some(x), y: Some(y), human })).await
    }

    pub async fn type_text(&self, execution_id: &str, text: String, selector: Option<String>) -> Result<(), AppError> {
        self.exec(execution_id, "", Action::TypeText(Type { text, selector })).await
    }

    pub async fn screenshot(&self, execution_id: &str) -> Result<Option<Vec<u8>>, AppError> {
        let result = self.exec_inner(execution_id, "", Action::Screenshot(Screenshot {
            quality: self.state.config.ai.resolution.quality,
            format: self.state.config.ai.resolution.format.clone(),
        })).await?;
        Ok(result.screenshot.map(|s| s.data))
    }

    pub async fn eval_js(&self, execution_id: &str, script: String) -> Result<(), AppError> {
        self.exec(execution_id, "", Action::EvalJs(EvalJs { script })).await
    }

    pub async fn get_execution_logs(&self, execution_id: &str) -> Result<String, AppError> {
        self.state.flux.get_execution_logs(execution_id).await
            .map_err(|e| AppError::Internal(format!("Flux logs: {e}")))
    }

    pub async fn dispatch(&self, execution_id: &str, action: &BrowserAction) -> Result<(), AppError> {
        let proto_action = match action {
            BrowserAction::Navigate { url } => Action::Navigate(
                Navigate { url: url.clone(), wait_until: "complete".to_string() },
            ),
            BrowserAction::Click { x, y, human } => Action::Click(
                Click { selector: None, x: Some(*x), y: Some(*y), human: *human },
            ),
            BrowserAction::Type { text, selector } => Action::TypeText(
                Type { text: text.clone(), selector: selector.clone() },
            ),
            BrowserAction::MouseMove { x, y } => Action::HumanMouseMove(
                HumanMouseMove { selector: None, x: Some(*x), y: Some(*y) },
            ),
            BrowserAction::Scroll { delta_x, delta_y } => Action::Scroll(
                Scroll { delta_x: *delta_x, delta_y: *delta_y },
            ),
            BrowserAction::Wait { ms } => {
                tokio::time::sleep(std::time::Duration::from_millis(*ms)).await;
                return Ok(());
            }
            BrowserAction::Screenshot | BrowserAction::Done { .. } => return Ok(()),
        };
        self.exec(execution_id, "", proto_action).await
    }

    async fn exec(&self, execution_id: &str, context_id: &str, action: Action) -> Result<(), AppError> {
        self.exec_inner(execution_id, context_id, action).await.map(|_| ())
    }

    async fn exec_inner(&self, execution_id: &str, context_id: &str, action: Action) -> Result<CommandResult, AppError> {
        let browser = self.get_browser(execution_id).await?;
        self.connect(&browser).await?
            .execute(tonic::Request::new(BrowserCommand {
                // browser_id identifies the browser on the agent side
                browser_id: browser.browser_id.clone(),
                context_id: context_id.to_string(),
                action: Some(action),
            }))
            .await
            .map(|r| r.into_inner())
            .map_err(|e| AppError::Internal(format!("gRPC: {e}")))
    }

    async fn connect(&self, browser: &BrowserInfo) -> Result<GrpcClient, AppError> {
        let cert_pem = self.state.redis.get_tls_cert().await?
            .ok_or_else(|| AppError::Internal("TLS cert not found — run /initialize first".into()))?;

        let tls = tonic::transport::ClientTlsConfig::new()
            .ca_certificate(tonic::transport::Certificate::from_pem(&cert_pem))
            .domain_name("rustmani-agent");

        let addr = format!("https://{}:{}", browser.host, browser.grpc_port);
        tonic::transport::Channel::from_shared(addr)
            .map_err(|e| AppError::Internal(format!("Invalid addr: {e}")))?
            .tls_config(tls)
            .map_err(|e| AppError::Internal(format!("TLS config: {e}")))?
            .connect()
            .await
            .map(GrpcClient::new)
            .map_err(|e| AppError::Internal(format!("Connect: {e}")))
    }
}

#[async_trait]
impl AIInstructor for BrowserService {
    fn state(&self) -> &Arc<AppState> {
        &self.state
    }

    async fn screenshot(&self, execution_id: &str) -> Result<Option<Vec<u8>>, AppError> {
        BrowserService::screenshot(self, execution_id).await
    }

    async fn dispatch(&self, execution_id: &str, action: &BrowserAction) -> Result<(), AppError> {
        BrowserService::dispatch(self, execution_id, action).await
    }
}
