use anyhow::Result;
use clap::Subcommand;

use crate::client::RustyClient;
use crate::config::CliConfig;

#[derive(Subcommand)]
pub enum BrowserCmd {
    /// Spawn a new browser agent
    Spawn {
        #[arg(long)]
        identity: Option<String>,
    },

    /// List all active browsers
    List,

    /// Get browser details
    Get {
        #[arg(long)]
        id: Option<String>,
    },

    /// Close and deregister a browser
    Close {
        #[arg(long)]
        id: Option<String>,
    },

    /// Close all browsers
    CloseAll,

    /// Create a new browsing context (tab)
    CreateContext {
        #[arg(long)]
        id: Option<String>,
    },

    /// Close a browsing context
    CloseContext {
        #[arg(long)]
        id: Option<String>,

        #[arg(long)]
        context_id: String,
    },

    /// Take a screenshot (prints base64 data URL)
    Screenshot {
        #[arg(long)]
        id: Option<String>,
    },

    /// Navigate to a URL
    Navigate {
        #[arg(long)]
        id: Option<String>,

        #[arg(long)]
        url: String,

        #[arg(long)]
        wait_until: Option<String>,
    },

    /// Click at pixel coordinates
    Click {
        #[arg(long)]
        id: Option<String>,

        #[arg(long)]
        x: f32,

        #[arg(long)]
        y: f32,
    },

    /// Click a DOM element by node_id (from `find-node`)
    NodeClick {
        #[arg(long)]
        id: Option<String>,

        #[arg(long)]
        node_id: i64,
    },

    /// Type text, optionally into a node
    Type {
        #[arg(long)]
        id: Option<String>,

        #[arg(long)]
        text: String,

        #[arg(long)]
        node_id: Option<i64>,
    },

    /// Scroll the page by pixels (positive = down)
    ScrollBy {
        #[arg(long)]
        id: Option<String>,

        #[arg(long)]
        y: i32,
    },

    /// Scroll a node into view
    ScrollTo {
        #[arg(long)]
        id: Option<String>,

        #[arg(long)]
        node_id: i64,
    },

    /// Send one or more key presses (e.g. "Backspace, Backspace")
    SendKeys {
        #[arg(long)]
        id: Option<String>,

        #[arg(long)]
        keys: String,
    },

    /// Hold a key for a duration (e.g. "Backspace3000")
    HoldKey {
        #[arg(long)]
        id: Option<String>,

        #[arg(long)]
        key: String,
    },

    /// Evaluate JavaScript and print the result
    Eval {
        #[arg(long)]
        id: Option<String>,

        #[arg(long)]
        script: String,
    },

    /// Fetch inner HTML of a node (omit node-id for full document)
    FetchHtml {
        #[arg(long)]
        id: Option<String>,

        #[arg(long)]
        node_id: Option<i64>,
    },

    /// Fetch inner text of a node
    FetchText {
        #[arg(long)]
        id: Option<String>,

        #[arg(long)]
        node_id: i64,
    },

    /// Find a node by CSS selector; prints node_id
    FindNode {
        #[arg(long)]
        id: Option<String>,

        #[arg(long)]
        selector: String,
    },

    /// Wait for a node to appear by CSS selector; prints node_id
    WaitForNode {
        #[arg(long)]
        id: Option<String>,

        #[arg(long)]
        selector: String,

        #[arg(long, default_value_t = 5000)]
        timeout_ms: u64,
    },

    /// Dump the accessibility tree for the current page
    UiMap {
        #[arg(long)]
        id: Option<String>,
    },

    /// Show what changed in the UI since the last ui-map call
    UiMapDiff {
        #[arg(long)]
        id: Option<String>,
    },

    /// Fetch execution logs from Flux
    Logs {
        #[arg(long)]
        id: Option<String>,
    },

    /// Run an AI instruction (async; use `logs` to follow)
    Instruct {
        #[arg(long)]
        id: Option<String>,

        #[arg(long)]
        instruction: String,
    },
}

fn resolve_id(id: Option<String>, cfg: &CliConfig) -> Result<String> {
    id.or_else(|| cfg.last_browser.clone())
        .ok_or_else(|| anyhow::anyhow!("No browser ID provided and no last browser stored. Run `browser spawn` first."))
}

pub fn handle(client: &RustyClient, cmd: BrowserCmd) -> Result<()> {
    let mut cfg = CliConfig::load();

    match cmd {
        BrowserCmd::Spawn { identity } => {
            let body = serde_json::json!({ "identity": identity });
            let resp: serde_json::Value = client.put("/browsers/", &body)?;
            if let Some(id) = resp.get("execution_id").and_then(|v| v.as_str()) {
                cfg.set_last_browser(id)?;
            }
            println!("{}", serde_json::to_string_pretty(&resp)?);
        }
        BrowserCmd::List => {
            let resp: serde_json::Value = client.get("/browsers/")?;
            println!("{}", serde_json::to_string_pretty(&resp)?);
        }
        BrowserCmd::Get { id } => {
            let id = resolve_id(id, &cfg)?;
            let resp: serde_json::Value = client.get(&format!("/browsers/{id}/"))?;
            println!("{}", serde_json::to_string_pretty(&resp)?);
        }
        BrowserCmd::Close { id } => {
            let id = resolve_id(id, &cfg)?;
            let resp: serde_json::Value = client.delete(&format!("/browsers/{id}/"))?;
            println!("{}", serde_json::to_string_pretty(&resp)?);
        }
        BrowserCmd::CloseAll => {
            let resp: serde_json::Value = client.delete("/browsers/")?;
            println!("{}", serde_json::to_string_pretty(&resp)?);
        }
        BrowserCmd::CreateContext { id } => {
            let id = resolve_id(id, &cfg)?;
            let resp: serde_json::Value = client.put(&format!("/browsers/{id}/contexts/"), &serde_json::json!({}))?;
            println!("{}", serde_json::to_string_pretty(&resp)?);
        }
        BrowserCmd::CloseContext { id, context_id } => {
            let id = resolve_id(id, &cfg)?;
            let resp: serde_json::Value = client.delete(&format!("/browsers/{id}/contexts/{context_id}/"))?;
            println!("{}", serde_json::to_string_pretty(&resp)?);
        }
        BrowserCmd::Screenshot { id } => {
            let id = resolve_id(id, &cfg)?;
            let resp: serde_json::Value = client.post(&format!("/browsers/{id}/screenshot/"), &serde_json::json!({}))?;
            println!("{}", serde_json::to_string_pretty(&resp)?);
        }
        BrowserCmd::Navigate { id, url, wait_until } => {
            let id = resolve_id(id, &cfg)?;
            let body = serde_json::json!({ "url": url, "wait_until": wait_until });
            let resp: serde_json::Value = client.post(&format!("/browsers/{id}/navigate/"), &body)?;
            println!("{}", serde_json::to_string_pretty(&resp)?);
        }
        BrowserCmd::Click { id, x, y } => {
            let id = resolve_id(id, &cfg)?;
            let resp: serde_json::Value = client.post(&format!("/browsers/{id}/click/"), &serde_json::json!({ "x": x, "y": y }))?;
            println!("{}", serde_json::to_string_pretty(&resp)?);
        }
        BrowserCmd::NodeClick { id, node_id } => {
            let id = resolve_id(id, &cfg)?;
            let resp: serde_json::Value = client.post(&format!("/browsers/{id}/node-click/"), &serde_json::json!({ "node_id": node_id }))?;
            println!("{}", serde_json::to_string_pretty(&resp)?);
        }
        BrowserCmd::Type { id, text, node_id } => {
            let id = resolve_id(id, &cfg)?;
            let resp: serde_json::Value = client.post(&format!("/browsers/{id}/type/"), &serde_json::json!({ "text": text, "node_id": node_id }))?;
            println!("{}", serde_json::to_string_pretty(&resp)?);
        }
        BrowserCmd::ScrollBy { id, y } => {
            let id = resolve_id(id, &cfg)?;
            let resp: serde_json::Value = client.post(&format!("/browsers/{id}/scroll-by/"), &serde_json::json!({ "y": y }))?;
            println!("{}", serde_json::to_string_pretty(&resp)?);
        }
        BrowserCmd::ScrollTo { id, node_id } => {
            let id = resolve_id(id, &cfg)?;
            let resp: serde_json::Value = client.post(&format!("/browsers/{id}/scroll-to/"), &serde_json::json!({ "node_id": node_id }))?;
            println!("{}", serde_json::to_string_pretty(&resp)?);
        }
        BrowserCmd::SendKeys { id, keys } => {
            let id = resolve_id(id, &cfg)?;
            let resp: serde_json::Value = client.post(&format!("/browsers/{id}/send-keys/"), &serde_json::json!({ "keys": keys }))?;
            println!("{}", serde_json::to_string_pretty(&resp)?);
        }
        BrowserCmd::HoldKey { id, key } => {
            let id = resolve_id(id, &cfg)?;
            let resp: serde_json::Value = client.post(&format!("/browsers/{id}/hold-key/"), &serde_json::json!({ "key": key }))?;
            println!("{}", serde_json::to_string_pretty(&resp)?);
        }
        BrowserCmd::Eval { id, script } => {
            let id = resolve_id(id, &cfg)?;
            let resp: serde_json::Value = client.post(&format!("/browsers/{id}/eval/"), &serde_json::json!({ "script": script }))?;
            println!("{}", serde_json::to_string_pretty(&resp)?);
        }
        BrowserCmd::FetchHtml { id, node_id } => {
            let id = resolve_id(id, &cfg)?;
            let resp: serde_json::Value = client.post(&format!("/browsers/{id}/fetch-html/"), &serde_json::json!({ "node_id": node_id }))?;
            println!("{}", serde_json::to_string_pretty(&resp)?);
        }
        BrowserCmd::FetchText { id, node_id } => {
            let id = resolve_id(id, &cfg)?;
            let resp: serde_json::Value = client.post(&format!("/browsers/{id}/fetch-text/"), &serde_json::json!({ "node_id": node_id }))?;
            println!("{}", serde_json::to_string_pretty(&resp)?);
        }
        BrowserCmd::FindNode { id, selector } => {
            let id = resolve_id(id, &cfg)?;
            let resp: serde_json::Value = client.post(&format!("/browsers/{id}/find-node/"), &serde_json::json!({ "selector": selector }))?;
            println!("{}", serde_json::to_string_pretty(&resp)?);
        }
        BrowserCmd::WaitForNode { id, selector, timeout_ms } => {
            let id = resolve_id(id, &cfg)?;
            let resp: serde_json::Value = client.post(&format!("/browsers/{id}/wait-for-node/"), &serde_json::json!({ "selector": selector, "timeout_ms": timeout_ms }))?;
            println!("{}", serde_json::to_string_pretty(&resp)?);
        }
        BrowserCmd::UiMap { id } => {
            let id = resolve_id(id, &cfg)?;
            let resp: serde_json::Value = client.get(&format!("/browsers/{id}/ui-map/"))?;
            println!("{}", serde_json::to_string_pretty(&resp)?);
        }
        BrowserCmd::UiMapDiff { id } => {
            let id = resolve_id(id, &cfg)?;
            let resp: serde_json::Value = client.get(&format!("/browsers/{id}/ui-map-diff/"))?;
            println!("{}", serde_json::to_string_pretty(&resp)?);
        }
        BrowserCmd::Logs { id } => {
            let id = resolve_id(id, &cfg)?;
            let resp: serde_json::Value = client.get(&format!("/browsers/{id}/logs/"))?;
            println!("{}", serde_json::to_string_pretty(&resp)?);
        }
        BrowserCmd::Instruct { id, instruction } => {
            let id = resolve_id(id, &cfg)?;
            let body = serde_json::json!({ "instruction": instruction });
            let resp: serde_json::Value = client.post(&format!("/browsers/{id}/instruct/"), &body)?;
            println!("{}", serde_json::to_string_pretty(&resp)?);
        }
    }

    Ok(())
}
