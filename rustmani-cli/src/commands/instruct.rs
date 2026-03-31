use anyhow::Result;
use clap::Subcommand;

use crate::client::RustmaniClient;

#[derive(Subcommand)]
pub enum InstructAction {
    /// Run an AI instruction on a browser
    Run {
        /// Browser ID
        #[arg(long)]
        browser: String,
        /// Instruction text
        instruction: String,
    },
}

pub async fn handle(client: &RustmaniClient, action: InstructAction) -> Result<()> {
    match action {
        InstructAction::Run { browser, instruction } => {
            let body = serde_json::json!({ "instruction": instruction });
            let resp: serde_json::Value =
                client.post(&format!("/browsers/{browser}/instruct"), &body).await?;
            println!("{}", serde_json::to_string_pretty(&resp)?);
        }
    }
    Ok(())
}
