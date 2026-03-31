use anyhow::Result;
use clap::{Parser, Subcommand};

mod client;
mod commands;
mod error;

#[derive(Parser)]
#[command(name = "rustmani-cli", about = "CLI for the rustmani browser orchestrator")]
struct Cli {
    /// Master server URL
    #[arg(long, env = "RUSTMANI_URL", default_value = "http://127.0.0.1:8080")]
    url: String,

    /// API key for authentication
    #[arg(long, env = "RUSTMANI_API_KEY")]
    api_key: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Manage browsers
    Browser {
        #[command(subcommand)]
        action: commands::browser::BrowserAction,
    },
    /// AI instruct a browser
    Instruct {
        #[command(subcommand)]
        action: commands::instruct::InstructAction,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "rustmani_cli=info".parse().unwrap()),
        )
        .init();

    let cli = Cli::parse();
    let client = client::RustmaniClient::new(&cli.url, &cli.api_key);

    match cli.command {
        Commands::Browser { action } => commands::browser::handle(&client, action).await?,
        Commands::Instruct { action } => commands::instruct::handle(&client, action).await?,
    }

    Ok(())
}
