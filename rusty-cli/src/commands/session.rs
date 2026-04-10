use anyhow::Result;
use clap::Subcommand;

use crate::client::RustyClient;

#[derive(Subcommand)]
pub enum SessionAction {
    /// Create a new session
    Create {
        /// Request exclusive access to the browser
        #[arg(long)]
        exclusive: bool,
    },
    /// List all sessions
    List,
    /// Get session details
    Get {
        /// Session ID
        id: String,
    },
    /// Release a session
    Release {
        /// Session ID
        id: String,
    },
}

pub async fn handle(client: &RustyClient, action: SessionAction) -> Result<()> {
    match action {
        SessionAction::Create { exclusive } => {
            let body = serde_json::json!({ "exclusive": exclusive });
            let resp: serde_json::Value = client.post("/sessions", &body).await?;
            println!("{}", serde_json::to_string_pretty(&resp)?);
        }
        SessionAction::List => {
            let resp: serde_json::Value = client.get("/sessions").await?;
            println!("{}", serde_json::to_string_pretty(&resp)?);
        }
        SessionAction::Get { id } => {
            let resp: serde_json::Value = client.get(&format!("/sessions/{id}")).await?;
            println!("{}", serde_json::to_string_pretty(&resp)?);
        }
        SessionAction::Release { id } => {
            let resp: serde_json::Value = client.delete(&format!("/sessions/{id}")).await?;
            println!("{}", serde_json::to_string_pretty(&resp)?);
        }
    }
    Ok(())
}
