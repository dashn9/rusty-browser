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
        /// Country code for proxy selection (e.g. US, GB)
        #[arg(long)]
        country: Option<String>,
    },
    /// List all active browsers
    List,
    /// Get browser details
    Get { id: Option<String> },
    /// Close and deregister a browser
    Close { id: Option<String> },
    /// Close all browsers
    CloseAll,
    /// Take a screenshot (prints base64)
    Screenshot { id: Option<String> },
    /// Navigate to a URL
    Navigate {
        id: Option<String>,
        url: String,
        #[arg(long)]
        wait_until: Option<String>,
    },
    /// Click at pixel coordinates
    Click {
        id: Option<String>,
        x: f32,
        y: f32,
    },
    /// Click a DOM element by CSS selector
    NodeClick {
        id: Option<String>,
        selector: String,
    },
    /// Type text, optionally into a selector
    Type {
        id: Option<String>,
        text: String,
        #[arg(long)]
        selector: Option<String>,
    },
    /// Scroll the page by pixels (positive = down)
    ScrollBy {
        id: Option<String>,
        y: i32,
    },
    /// Evaluate JavaScript and print the result
    Eval {
        id: Option<String>,
        script: String,
    },
    /// Fetch inner HTML of a selector
    FetchHtml {
        id: Option<String>,
        #[arg(long)]
        selector: Option<String>,
    },
    /// Fetch inner text of a selector
    FetchText {
        id: Option<String>,
        selector: String,
    },
    /// Fetch execution logs from Flux
    Logs { id: Option<String> },
    /// Run an AI instruction
    Instruct {
        id: Option<String>,
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
        BrowserCmd::Spawn { identity, country } => {
            let body = serde_json::json!({ "identity": identity, "country": country });
            let resp: serde_json::Value = client.put("/browsers/", &body)?;
            // Store the returned execution_id as last browser
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
        BrowserCmd::NodeClick { id, selector } => {
            let id = resolve_id(id, &cfg)?;
            let resp: serde_json::Value = client.post(&format!("/browsers/{id}/node-click/"), &serde_json::json!({ "selector": selector }))?;
            println!("{}", serde_json::to_string_pretty(&resp)?);
        }
        BrowserCmd::Type { id, text, selector } => {
            let id = resolve_id(id, &cfg)?;
            let resp: serde_json::Value = client.post(&format!("/browsers/{id}/type/"), &serde_json::json!({ "text": text, "selector": selector }))?;
            println!("{}", serde_json::to_string_pretty(&resp)?);
        }
        BrowserCmd::ScrollBy { id, y } => {
            let id = resolve_id(id, &cfg)?;
            let resp: serde_json::Value = client.post(&format!("/browsers/{id}/scroll-by/"), &serde_json::json!({ "y": y }))?;
            println!("{}", serde_json::to_string_pretty(&resp)?);
        }
        BrowserCmd::Eval { id, script } => {
            let id = resolve_id(id, &cfg)?;
            let resp: serde_json::Value = client.post(&format!("/browsers/{id}/eval/"), &serde_json::json!({ "script": script }))?;
            println!("{}", serde_json::to_string_pretty(&resp)?);
        }
        BrowserCmd::FetchHtml { id, selector } => {
            let id = resolve_id(id, &cfg)?;
            let resp: serde_json::Value = client.post(&format!("/browsers/{id}/fetch-html/"), &serde_json::json!({ "selector": selector }))?;
            println!("{}", serde_json::to_string_pretty(&resp)?);
        }
        BrowserCmd::FetchText { id, selector } => {
            let id = resolve_id(id, &cfg)?;
            let resp: serde_json::Value = client.post(&format!("/browsers/{id}/fetch-text/"), &serde_json::json!({ "selector": selector }))?;
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
