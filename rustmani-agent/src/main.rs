use clap::Parser;
use tracing::info;

mod browser;
mod error;
mod executor;
mod server;

#[derive(Parser)]
struct Args {
    #[arg(long)]
    browser_id: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "rustmani_agent=info".parse().unwrap()),
        )
        .init();

    let args = Args::parse();
    let browser_id = args.browser_id;

    let browser_config = browser::ChromeBrowserLaunchConfig::from_env().unwrap_or_default();

    info!("Starting rustmani-agent {browser_id}");

    let browser = browser::ManagedBrowser::launch(browser_config).await?;

    server::serve(browser, &browser_id).await?;

    Ok(())
}
