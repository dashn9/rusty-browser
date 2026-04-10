use anyhow::Result;
use clap::Subcommand;

use crate::client::RustyClient;

#[derive(Subcommand)]
pub enum BrowserAction {
    /// Create a new browser instance
    Create {
        /// Identity JSON file path
        #[arg(long)]
        identity: Option<String>,
    },
    /// List all browsers
    List,
    /// Get browser details
    Get {
        /// Browser ID
        id: String,
    },
    /// Close and remove a browser
    Close {
        /// Browser ID
        id: String,
    },
}

pub async fn handle(client: &RustyClient, action: BrowserAction) -> Result<()> {
    match action {
        BrowserAction::Create { identity } => {
            let resp: serde_json::Value = client.post("/browsers", &body).await?;
            println!("{}", serde_json::to_string_pretty(&resp)?);
        }
        BrowserAction::List => {
            let resp: serde_json::Value = client.get("/browsers").await?;
            println!("{}", serde_json::to_string_pretty(&resp)?);
        }
        BrowserAction::Get { id } => {
            let resp: serde_json::Value = client.get(&format!("/browsers/{id}")).await?;
            println!("{}", serde_json::to_string_pretty(&resp)?);
        }
        BrowserAction::Close { id } => {
            let resp: serde_json::Value = client.delete(&format!("/browsers/{id}")).await?;
            println!("{}", serde_json::to_string_pretty(&resp)?);
        }
    }
    Ok(())
}
