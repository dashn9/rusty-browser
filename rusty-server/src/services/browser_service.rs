use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use uuid::Uuid;

use rusty_common::ai::BrowserAction;
use rusty_common::error::BrowserError;
use rusty_common::state::{BrowserInfo, BrowserState};
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

    /// Spawns a new agent via Flux (or locally if `flux.local_binary` is set).
    /// Returns the execution_id — the agent registers back via gRPC.
    pub async fn create_browser(&self, identity: Option<serde_json::Value>) -> Result<String, AppError> {
        let master_url = self.state.config.server.grpc_server_url.clone().unwrap_or_else(|| {
            format!(
                "https://{}:{}",
                self.state.public_ip,
                self.state.config.server.grpc_port.expect("grpc_port resolved at startup"),
            )
        });

        if let Some(ref binary) = self.state.config.flux.local_binary {
            let execution_id = format!("test-{}", Uuid::new_v4());
            self.state.redis.store_pending_execution(&execution_id).await?;
            spawn_local_agent(binary, &execution_id, &master_url, &self.state.config.agent_env).await?;
            tracing::info!("local agent spawned execution_id={execution_id}");
            return Ok(execution_id);
        }

        // master_url is argv[1]; any identity args follow
        let mut args = vec![master_url.clone()];
        if self.state.config.server.grpc_server_url.is_some() {
            args.push("--native-tls".to_string());
        }
        args.extend(identity.into_iter().map(|v| v.to_string()));
        tracing::info!("spawning agent master_url={master_url}");
        let execution_id = self.state.flux.spawn_agent(&self.state.config.flux.function_name, &args).await?;
        self.state.redis.store_pending_execution(&execution_id).await?;
        tracing::info!("agent spawned execution_id={execution_id}");
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
        tracing::info!("delete execution_id={execution_id}");
        // Agent calls process::exit(0) on CloseBrowser — it may exit before replying
        let _ = self.exec(execution_id, "", Action::CloseBrowser(CloseBrowser {})).await;
        self.state.redis.remove_browser(execution_id).await?;
        tracing::info!("deleted execution_id={execution_id}");
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

    pub async fn type_text(&self, execution_id: &str, text: String, node_id: Option<i64>) -> Result<(), AppError> {
        self.exec(execution_id, "", Action::TypeText(Type { text, node_id })).await
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

    pub async fn node_click(&self, execution_id: &str, node_id: i64, human: bool) -> Result<(), AppError> {
        self.exec(execution_id, "", Action::NodeClick(NodeClick { node_id, human })).await
    }

    pub async fn fetch_html(&self, execution_id: &str, node_id: Option<i64>) -> Result<String, AppError> {
        let r = self.exec_inner(execution_id, "", Action::FetchHtml(FetchHtml { node_id })).await?;
        Ok(r.result)
    }

    pub async fn fetch_text(&self, execution_id: &str, node_id: i64) -> Result<String, AppError> {
        let r = self.exec_inner(execution_id, "", Action::FetchText(FetchText { node_id })).await?;
        Ok(r.result)
    }

    pub async fn eval_js(&self, execution_id: &str, script: String) -> Result<String, AppError> {
        let r = self.exec_inner(execution_id, "", Action::EvalJs(EvalJs { script })).await?;
        Ok(r.result)
    }

    pub async fn scroll_by(&self, execution_id: &str, y: i32, human: bool) -> Result<(), AppError> {
        self.exec(execution_id, "", Action::ScrollBy(ScrollBy { y, human })).await
    }

    pub async fn scroll_to(&self, execution_id: &str, node_id: i64, human: bool) -> Result<(), AppError> {
        self.exec(execution_id, "", Action::ScrollTo(ScrollTo { node_id, human })).await
    }

    pub async fn send_keys(&self, execution_id: &str, keys: &str) -> Result<(), AppError> {
        self.exec(execution_id, "", Action::SendKeys(SendKeys { keys: parse_keys(keys) })).await
    }

    pub async fn hold_key(&self, execution_id: &str, input: &str) -> Result<(), AppError> {
        let (key, duration_ms) = parse_hold_key(input);
        self.exec(execution_id, "", Action::HoldKey(HoldKey { key, duration_ms })).await
    }

    pub async fn teardown(&self) -> Result<serde_json::Value, AppError> {
        let browsers = self.delete_all_browsers().await?;
        let nodes_terminated = self.state.flux.terminate_all_nodes().await.is_ok();
        Ok(serde_json::json!({
            "browsers": browsers,
            "nodes_terminated": nodes_terminated,
        }))
    }

    pub async fn find_node(&self, execution_id: &str, selector: String) -> Result<i64, AppError> {
        let r = self.exec_inner(execution_id, "", Action::FindNode(FindNode { selector })).await?;
        r.result.parse::<i64>().map_err(|e| AppError::Internal(format!("node_id parse: {e}")))
    }

    pub async fn wait_for_node(&self, execution_id: &str, selector: String, timeout_ms: u64) -> Result<i64, AppError> {
        let r = self.exec_inner(execution_id, "", Action::WaitForNode(WaitForNode { selector, timeout_ms })).await?;
        r.result.parse::<i64>().map_err(|e| AppError::Internal(format!("node_id parse: {e}")))
    }

    pub async fn get_ui_map(&self, execution_id: &str) -> Result<Vec<rusty_common::ui_map::UiNode>, AppError> {
        let r = self.exec_inner(execution_id, "", Action::GetUiMap(GetUiMap {})).await?;
        let nodes: Vec<rusty_common::ui_map::UiNode> = serde_json::from_str(&r.result)
            .map_err(|e| AppError::Internal(format!("ui_map parse: {e}")))?;
        if let Ok(mut cache) = self.state.ui_map_cache.lock() {
            cache.insert(execution_id.to_string(), nodes.clone());
        }
        Ok(nodes)
    }

    pub async fn engage_input(&self, execution_id: &str, node_id: i64, value: &str) -> Result<String, AppError> {
        if value.is_empty() {
            let _ = self.scroll_to(execution_id, node_id, true).await;
            self.node_click(execution_id, node_id, true).await?;
            return Ok(format!("clicked node {node_id}"));
        }

        let role = self.state.ui_map_cache.lock().ok()
            .and_then(|c| c.get(execution_id)
                .and_then(|nodes| nodes.iter().find(|n| n.id == node_id).map(|n| n.role.clone())))
            .unwrap_or_default();

        if matches!(role.to_lowercase().as_str(), "combobox" | "listbox" | "select") {
            let val_lower = value.to_lowercase();
            const MAX_ATTEMPTS: u32 = 2;
            for attempt in 0..MAX_ATTEMPTS {
                // snapshot before click so diff only shows nodes that appeared from this open
                self.get_ui_map(execution_id).await?;
                if attempt == 0 {
                    let _ = self.scroll_to(execution_id, node_id, true).await;
                }
                self.node_click(execution_id, node_id, true).await?;
                let diff = self.get_ui_map_diff(execution_id).await?;
                let options: Vec<_> = diff.added.iter()
                    .filter(|n| n.role.to_lowercase() == "option")
                    .collect();
                if options.is_empty() {
                    continue;
                }
                let opt = options.iter().copied()
                    .find(|n| n.name.as_ref().map(|s| s.to_lowercase() == val_lower).unwrap_or(false))
                    .or_else(|| options.iter().copied()
                        .find(|n| n.name.as_ref().map(|s| s.to_lowercase().contains(&val_lower)).unwrap_or(false)));
                match opt {
                    Some(o) => {
                        self.node_click(execution_id, o.id, true).await?;
                        return Ok(format!("selected '{}' from combobox {node_id}", o.name.as_deref().unwrap_or("")));
                    }
                    None => return Ok(format!("combobox {node_id} opened but no option matching '{value}' found")),
                }
            }
            Ok(format!("combobox {node_id} did not reveal options after {MAX_ATTEMPTS} attempts"))
        } else {
            let _ = self.scroll_to(execution_id, node_id, true).await;
            self.node_click(execution_id, node_id, true).await?;
            self.type_text(execution_id, value.to_string(), None).await?;
            Ok(format!("typed '{value}' into node {node_id}"))
        }
    }

    pub async fn get_ui_map_diff(&self, execution_id: &str) -> Result<rusty_common::ui_map::UiMapDiff, AppError> {
        let before = self.state.ui_map_cache.lock().ok()
            .and_then(|c| c.get(execution_id).cloned())
            .unwrap_or_default();
        let after = self.get_ui_map(execution_id).await?;
        Ok(rusty_common::ui_map::diff(&before, &after))
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
            BrowserAction::NodeClick { node_id, human } => {
                self.exec(execution_id, "", Action::NodeClick(NodeClick { node_id: *node_id, human: *human })).await?;
                Ok("ok".to_string())
            }
            BrowserAction::Type { text, node_id } => {
                self.exec(execution_id, "", Action::TypeText(Type { text: text.clone(), node_id: *node_id })).await?;
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
            BrowserAction::ScrollTo { node_id, human } => {
                self.exec(execution_id, "", Action::ScrollTo(ScrollTo { node_id: *node_id, human: *human })).await?;
                Ok("ok".to_string())
            }
            BrowserAction::FetchHtml { node_id } => {
                let r = self.exec_inner(execution_id, "", Action::FetchHtml(FetchHtml { node_id: *node_id })).await?;
                Ok(r.result)
            }
            BrowserAction::FetchText { node_id } => {
                let r = self.exec_inner(execution_id, "", Action::FetchText(FetchText { node_id: *node_id })).await?;
                Ok(r.result)
            }
            BrowserAction::EvalJs { script } => {
                let r = self.exec_inner(execution_id, "", Action::EvalJs(EvalJs { script: script.clone() })).await?;
                Ok(r.result)
            }
            BrowserAction::FindNode { selector } => {
                let r = self.exec_inner(execution_id, "", Action::FindNode(FindNode { selector: selector.clone() })).await?;
                Ok(r.result)  // node_id as decimal string
            }
            BrowserAction::WaitForNode { selector, timeout_ms } => {
                let r = self.exec_inner(execution_id, "", Action::WaitForNode(WaitForNode { selector: selector.clone(), timeout_ms: *timeout_ms })).await?;
                Ok(r.result)  // node_id as decimal string
            }
            BrowserAction::Wait { ms } => {
                tokio::time::sleep(std::time::Duration::from_millis(*ms)).await;
                Ok("ok".to_string())
            }
            BrowserAction::Screenshot => {
                BrowserService::screenshot(self, execution_id).await
            }
            BrowserAction::GetUiMap => {
                let r = self.exec_inner(execution_id, "", Action::GetUiMap(GetUiMap {})).await?;
                let nodes: Vec<rusty_common::ui_map::UiNode> = serde_json::from_str(&r.result)
                    .map_err(|e| AppError::Internal(format!("ui_map parse: {e}")))?;
                Ok(rusty_common::ui_map::format_compact(&nodes))
            }
            BrowserAction::GetUiMapDiff => {
                let d = self.get_ui_map_diff(execution_id).await?;
                let mut parts = Vec::new();
                for n in &d.added { parts.push(format!("+ {}", rusty_common::ui_map::format_compact(&[n.clone()]))); }
                for n in &d.changed { parts.push(format!("~ {}", rusty_common::ui_map::format_compact(&[n.clone()]))); }
                for n in &d.removed { parts.push(format!("- {}", rusty_common::ui_map::format_compact(&[n.clone()]))); }
                Ok(if parts.is_empty() { "no changes".to_string() } else { parts.join("\n") })
            }
            BrowserAction::SendKeys { keys } => {
                self.send_keys(execution_id, keys).await?;
                Ok("ok".to_string())
            }
            BrowserAction::HoldKey { key } => {
                self.hold_key(execution_id, key).await?;
                Ok("ok".to_string())
            }
            BrowserAction::Done { .. } => Ok("ok".to_string()),
            BrowserAction::EngageInput { node_id, value } => {
                self.engage_input(execution_id, *node_id, value).await
            }
        }
    }

    async fn exec(&self, execution_id: &str, context_id: &str, action: Action) -> Result<(), AppError> {
        self.exec_inner(execution_id, context_id, action).await.map(|_| ())
    }

    async fn exec_inner(&self, execution_id: &str, context_id: &str, action: Action) -> Result<CommandResult, AppError> {
        let browser = self.get_browser(execution_id).await?;
        tracing::debug!("exec execution_id={execution_id} action={:?}", action);
        let result = self.connect(&browser).await?
            .execute(tonic::Request::new(BrowserCommand {
                browser_id: browser.browser_id.clone(),
                context_id: context_id.to_string(),
                action: Some(action),
            }))
            .await
            .map(|r| r.into_inner())
            .map_err(|e| AppError::Internal(format!("gRPC: {e}")))?;
        if !result.success {
            tracing::error!("exec failed execution_id={execution_id}: {}", result.error_message);
        }
        Ok(result)
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
                        tracing::warn!("[TEST-AGENT] ==> Connect exhausted for {} ({}), removing: {e}", browser.execution_id, addr);
                        let _ = self.state.redis.remove_browser(&browser.execution_id).await;
                        return Err(AppError::Internal(format!("Connect {addr}: {e}")));
                    }
                    tracing::warn!("[TEST-AGENT] ==> Connect attempt {} failed for {} ({}): {e}", attempt + 1, browser.execution_id, addr);
                    tokio::time::sleep(RETRY_DELAY).await;
                }
            }
        }
        unreachable!()
    }
}

async fn spawn_local_agent(binary: &str, execution_id: &str, master_url: &str, agent_env: &std::collections::HashMap<String, String>) -> Result<(), AppError> {
    let mut parts = binary.split_whitespace();
    let program = parts.next()
        .ok_or_else(|| AppError::Internal("flux.local_binary is empty".into()))?;
    let mut cmd = Command::new(program);
    cmd.args(parts);
    cmd.arg(master_url);

    // Apply user-supplied env first so authoritative vars below cannot be overridden.
    cmd.envs(agent_env);
    cmd.env("FLUX_EXECUTION_ID", execution_id);
    cmd.env("FLUX_NODE_PRIVATE_IP", "127.0.0.1");
    cmd.env("FLUX_NODE_PUBLIC_IP", "127.0.0.1");

    // Only default RUSTY_CERT_DIR if neither the caller nor the parent env provided one.
    if !agent_env.contains_key("RUSTY_CERT_DIR") && std::env::var_os("RUSTY_CERT_DIR").is_none() {
        cmd.env("RUSTY_CERT_DIR", std::env::temp_dir());
    }

    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    let mut child = cmd.spawn()
        .map_err(|e| AppError::Internal(format!("local agent spawn: {e}")))?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    if let Some(stdout) = stdout {
        let id = execution_id.to_string();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                eprintln!("[TEST-AGENT] ==> [{id}] {line}");
            }
        });
    }
    if let Some(stderr) = stderr {
        let id = execution_id.to_string();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                eprintln!("[TEST-AGENT] ==> [{id}] {line}");
            }
        });
    }

    // Own the child so it gets reaped on exit (agent calls process::exit on CloseBrowser).
    let id = execution_id.to_string();
    tokio::spawn(async move {
        match child.wait().await {
            Ok(status) => eprintln!("[TEST-AGENT] ==> [{id}] exited: {status}"),
            Err(e) => eprintln!("[TEST-AGENT] ==> [{id}] wait failed: {e}"),
        }
    });
    Ok(())
}

impl AIInstructor for BrowserService {
    fn state(&self) -> &Arc<AppState> {
        &self.state
    }

    async fn dispatch(&self, execution_id: &str, action: &BrowserAction) -> Result<String, AppError> {
        BrowserService::dispatch(self, execution_id, action).await
    }
}

fn parse_hold_key(input: &str) -> (String, u64) {
    let input = input.trim();
    let digit_start = input.find(|c: char| c.is_ascii_digit()).unwrap_or(input.len());
    let (key, ms_str) = input.split_at(digit_start);
    let duration_ms = ms_str.parse::<u64>().unwrap_or(0);
    (key.to_string(), duration_ms)
}

fn parse_keys(input: &str) -> Vec<String> {
    let tokens: Vec<&str> = if input.contains(", ") {
        input.split(", ").collect()
    } else if input.contains(',') {
        input.split(',').collect()
    } else if input.contains(' ') {
        input.split(' ').collect()
    } else {
        vec![input]
    };

    let mut out = Vec::new();
    for token in tokens {
        let token = token.trim();
        if token.is_empty() { continue; }
        let digit_start = token.find(|c: char| c.is_ascii_digit()).unwrap_or(token.len());
        let (key, count_str) = token.split_at(digit_start);
        let count = count_str.parse::<usize>().unwrap_or(1).max(1);
        for _ in 0..count {
            out.push(key.to_string());
        }
    }
    out
}
