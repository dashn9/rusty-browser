use anyhow::Result;
use clap::{Parser, Subcommand};

mod client;
mod commands;
mod config;
mod error;

use config::CliConfig;

#[derive(Parser)]
#[command(name = "rusty-cli", about = "CLI for the Rusty browser orchestrator")]
struct Cli {
    #[arg(long, env = "RUSTY_URL")]
    url: Option<String>,

    #[arg(long, env = "RUSTY_API_KEY")]
    api_key: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize the server (generate certs, register agent function)
    Init,
    /// Tear down all browsers and terminate all Flux nodes
    Teardown,
    /// Manage and interact with browsers
    Browser {
        #[command(subcommand)]
        cmd: commands::browser::BrowserCmd,
    },
    /// Manage CLI configuration
    Env {
        #[command(subcommand)]
        cmd: EnvCmd,
    },
}

#[derive(Subcommand)]
enum EnvCmd {
    /// Set a config value (url, api-key)
    Set { key: String, value: String },
    /// Show current config
    Show,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let mut cfg = CliConfig::load();

    // CLI flags > env vars > stored config
    let url = cli.url
        .or_else(|| cfg.url.clone())
        .unwrap_or_else(|| "http://127.0.0.1:8080".to_string());

    let api_key = cli.api_key
        .or_else(|| cfg.api_key.clone())
        .unwrap_or_default();

    match cli.command {
        Commands::Env { cmd } => match cmd {
            EnvCmd::Set { key, value } => {
                match key.as_str() {
                    "url" => cfg.url = Some(value),
                    "api-key" => cfg.api_key = Some(value),
                    other => anyhow::bail!("Unknown config key '{other}'. Valid keys: url, api-key"),
                }
                cfg.save()?;
                println!("Saved.");
            }
            EnvCmd::Show => {
                println!("url:          {}", cfg.url.as_deref().unwrap_or("(not set)"));
                println!("api-key:      {}", cfg.api_key.as_deref().unwrap_or("(not set)"));
                println!("last-browser: {}", cfg.last_browser.as_deref().unwrap_or("(none)"));
                println!("config file:  {}", CliConfig::path().display());
            }
        },
        Commands::Init => {
            let client = client::RustyClient::new(&url, &api_key);
            let resp: serde_json::Value = client.post("/initialize/", &serde_json::json!({}))?;
            println!("{}", serde_json::to_string_pretty(&resp)?);
        }
        Commands::Teardown => {
            let client = client::RustyClient::new(&url, &api_key);
            let resp: serde_json::Value = client.delete("/teardown/")?;
            println!("{}", serde_json::to_string_pretty(&resp)?);
        }
        Commands::Browser { cmd } => {
            let client = client::RustyClient::new(&url, &api_key);
            commands::browser::handle(&client, cmd)?;
        }
    }

    Ok(())
}
