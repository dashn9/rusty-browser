use std::sync::Arc;

use base64::Engine;

use rusty_common::ai::BrowserAction;
use rusty_common::error::BrowserError;
use rusty_common::state::BrowserInfo;
use rusty_proto::browser_agent_client::BrowserAgentClient;
use rusty_proto::browser_command::Action;
use rusty_proto::*;

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
        let master_url = self.state.config.server.grpc_server_url.clone().unwrap_or_else(|| {
            format!(
                "https://{}:{}",
                self.state.public_ip,
                self.state.config.server.grpc_port.expect("grpc_port resolved at startup"),
            )
        });
        // master_url is argv[1]; any identity args follow
        let mut args = vec![master_url.clone()];
        if self.state.config.server.grpc_server_url.is_some() {
            args.push("--native-tls".to_string());
        }
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
        // Agent calls process::exit(0) on CloseBrowser — it may exit before replying
        let _ = self.exec(execution_id, "", Action::CloseBrowser(CloseBrowser {})).await;
        let _ = self.state.redis.clear_instruct(execution_id).await;
        self.state.redis.remove_browser(execution_id).await?;
        Ok(())
    }

    pub async fn delete_all_browsers(&self) -> Result<Vec<serde_json::Value>, AppError> {
        let browsers = self.state.redis.list_browsers().await?;
        let mut set = tokio::task::JoinSet::new();
        for b in &browsers {
            let svc = BrowserService::new(self.state.clone());
            let browser = b.clone();
            set.spawn(async move {
                let ok = svc.delete_browser(&browser.execution_id).await.is_ok();
                serde_json::json!({
                    "execution_id": browser.execution_id,
                    "browser_id": browser.browser_id,
                    "public_ip": browser.public_ip,
                    "private_ip": browser.private_ip,
                    "contexts": browser.contexts,
                    "deleted": ok,
                })
            });
        }
        let mut log = Vec::new();
        while let Some(res) = set.join_next().await {
            if let Ok(entry) = res {
                log.push(entry);
            }
        }
        Ok(log)
    }

    pub async fn create_context(&self, execution_id: &str) -> Result<String, AppError> {
        let r = self.exec_inner(execution_id, "", Action::CreateContext(CreateContext { url: None })).await?;
        let context_id = r.result;
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
        self.exec(execution_id, "", Action::Click(Click { x: Some(x), y: Some(y), human })).await
    }

    pub async fn type_text(&self, execution_id: &str, text: String, selector: Option<String>) -> Result<(), AppError> {
        self.exec(execution_id, "", Action::TypeText(Type { text, selector })).await
    }

    pub async fn screenshot(&self, execution_id: &str) -> Result<String, AppError> {
        let cmd_result = self.exec_inner(execution_id, "", Action::Screenshot(Screenshot {
            quality: self.state.config.ai.resolution.quality,
            format: "image/jpeg".to_string(),
        })).await?;
        if cmd_result.result.is_empty() {
            return Err(AppError::Internal("agent returned empty screenshot".into()));
        }
        Ok(cmd_result.result)
    }

    pub async fn node_click(&self, execution_id: &str, selector: String, human: bool) -> Result<(), AppError> {
        self.exec(execution_id, "", Action::NodeClick(NodeClick { selector, human })).await
    }

    pub async fn fetch_html(&self, execution_id: &str, selector: Option<String>) -> Result<String, AppError> {
        let r = self.exec_inner(execution_id, "", Action::FetchHtml(FetchHtml { selector })).await?;
        Ok(r.result)
    }

    pub async fn fetch_text(&self, execution_id: &str, selector: String) -> Result<String, AppError> {
        let r = self.exec_inner(execution_id, "", Action::FetchText(FetchText { selector })).await?;
        Ok(r.result)
    }

    pub async fn eval_js(&self, execution_id: &str, script: String) -> Result<String, AppError> {
        let r = self.exec_inner(execution_id, "", Action::EvalJs(EvalJs { script })).await?;
        Ok(r.result)
    }

    pub async fn scroll_by(&self, execution_id: &str, y: i32, human: bool) -> Result<(), AppError> {
        self.exec(execution_id, "", Action::ScrollBy(ScrollBy { y, human })).await
    }

    pub async fn scroll_to(&self, execution_id: &str, selector: String, human: bool, to: u32) -> Result<(), AppError> {
        self.exec(execution_id, "", Action::ScrollTo(ScrollTo { selector, human, to })).await
    }

    pub async fn teardown(&self) -> Result<serde_json::Value, AppError> {
        let browsers = self.delete_all_browsers().await?;
        let nodes_terminated = self.state.flux.terminate_all_nodes().await.is_ok();
        Ok(serde_json::json!({
            "browsers": browsers,
            "nodes_terminated": nodes_terminated,
        }))
    }

    pub async fn get_execution_logs(&self, execution_id: &str) -> Result<String, AppError> {
        self.state.flux.get_execution_logs(execution_id).await
            .map_err(|e| AppError::Internal(format!("Flux logs: {e}")))
    }

    pub async fn dispatch(&self, execution_id: &str, action: &BrowserAction) -> Result<String, AppError> {
        match action {
            BrowserAction::Navigate { url } => {
                self.exec(execution_id, "", Action::Navigate(Navigate { url: url.clone(), wait_until: "complete".to_string() })).await?;
                Ok("ok".to_string())
            }
            BrowserAction::Click { x, y, human } => {
                self.exec(execution_id, "", Action::Click(Click { x: Some(*x), y: Some(*y), human: *human })).await?;
                Ok("ok".to_string())
            }
            BrowserAction::NodeClick { selector, human } => {
                self.exec(execution_id, "", Action::NodeClick(NodeClick { selector: selector.clone(), human: *human })).await?;
                Ok("ok".to_string())
            }
            BrowserAction::Type { text, selector } => {
                self.exec(execution_id, "", Action::TypeText(Type { text: text.clone(), selector: selector.clone() })).await?;
                Ok("ok".to_string())
            }
            BrowserAction::MouseMove { x, y } => {
                self.exec(execution_id, "", Action::MouseMove(MouseMove { x: Some(*x), y: Some(*y), steps: 0 })).await?;
                Ok("ok".to_string())
            }
            BrowserAction::HumanMouseMove { x, y } => {
                self.exec(execution_id, "", Action::HumanMouseMove(HumanMouseMove { x: Some(*x), y: Some(*y) })).await?;
                Ok("ok".to_string())
            }
            BrowserAction::ScrollBy { y, human } => {
                self.exec(execution_id, "", Action::ScrollBy(ScrollBy { y: *y, human: *human })).await?;
                Ok("ok".to_string())
            }
            BrowserAction::ScrollTo { selector, human, to } => {
                self.exec(execution_id, "", Action::ScrollTo(ScrollTo { selector: selector.clone(), human: *human, to: *to })).await?;
                Ok("ok".to_string())
            }
            BrowserAction::FetchHtml { selector } => {
                let r = self.exec_inner(execution_id, "", Action::FetchHtml(FetchHtml { selector: selector.clone() })).await?;
                Ok(r.result)
            }
            BrowserAction::FetchText { selector } => {
                let r = self.exec_inner(execution_id, "", Action::FetchText(FetchText { selector: selector.clone() })).await?;
                Ok(r.result)
            }
            BrowserAction::EvalJs { script } => {
                let r = self.exec_inner(execution_id, "", Action::EvalJs(EvalJs { script: script.clone() })).await?;
                Ok(r.result)
            }
            BrowserAction::FindNode { selector } => {
                let r = self.exec_inner(execution_id, "", Action::FindNode(FindNode { selector: selector.clone() })).await?;
                Ok(r.result)
            }
            BrowserAction::WaitForNode { selector, timeout_ms } => {
                let r = self.exec_inner(execution_id, "", Action::WaitForNode(WaitForNode { selector: selector.clone(), timeout_ms: *timeout_ms })).await?;
                Ok(r.result)
            }
            BrowserAction::Wait { ms } => {
                tokio::time::sleep(std::time::Duration::from_millis(*ms)).await;
                Ok("ok".to_string())
            }
            BrowserAction::Screenshot => {
                BrowserService::screenshot(self, execution_id).await
            }
            BrowserAction::Done { .. } => Ok("ok".to_string()),
        }
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
        const MAX_RETRIES: u32 = 3;
        const RETRY_DELAY: std::time::Duration = std::time::Duration::from_millis(500);

        let cert_pem = self.state.redis.get_tls_cert().await?
            .ok_or_else(|| AppError::Internal("TLS cert not found — run /initialize first".into()))?;

        let addr = format!("https://{}:{}", browser.public_ip, browser.grpc_port);

        for attempt in 0..MAX_RETRIES {
            let tls = tonic::transport::ClientTlsConfig::new()
                .ca_certificate(tonic::transport::Certificate::from_pem(&cert_pem))
                .domain_name("rusty-agent");

            match tonic::transport::Channel::from_shared(addr.clone())
                .map_err(|e| AppError::Internal(format!("Invalid addr: {e}")))?
                .tls_config(tls)
                .map_err(|e| AppError::Internal(format!("TLS config: {e}")))?
                .connect()
                .await
            {
                Ok(ch) => return Ok(GrpcClient::new(ch)),
                Err(e) => {
                    if attempt + 1 == MAX_RETRIES {
                        tracing::warn!("Connect exhausted for {}, removing: {e}", browser.execution_id);
                        let _ = self.state.redis.clear_instruct(&browser.execution_id).await;
                        let _ = self.state.redis.remove_browser(&browser.execution_id).await;
                        return Err(AppError::Internal(format!("Connect: {e}")));
                    }
                    tracing::warn!("Connect attempt {} failed for {}: {e}", attempt + 1, browser.execution_id);
                    tokio::time::sleep(RETRY_DELAY).await;
                }
            }
        }
        unreachable!()
    }
}

impl AIInstructor for BrowserService {
    fn state(&self) -> &Arc<AppState> {
        &self.state
    }

    async fn dispatch(&self, execution_id: &str, action: &BrowserAction) -> Result<String, AppError> {
        BrowserService::dispatch(self, execution_id, action).await
    }
}
