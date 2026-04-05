use tracing::info;
use uuid::Uuid;

mod browser;
mod error;
mod executor;
mod server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "rustmani_agent=info".parse().unwrap()),
        )
        .init();

    let execution_id = std::env::var("FLUX_EXECUTION_ID")
        .expect("FLUX_EXECUTION_ID must be set by Flux");

    let master_url = std::env::var("RUSTMANI_MASTER_URL")
        .expect("RUSTMANI_MASTER_URL must be set");

    let browser_id = Uuid::new_v4().to_string();
    let browser_config = browser::ChromeBrowserLaunchConfig::from_env().unwrap_or_default();

    info!("Starting rustmani-agent browser={browser_id} execution={execution_id}");

    let browser = browser::ManagedBrowser::launch(browser_config).await?;

    server::serve(browser, &browser_id, &execution_id, &master_url).await?;

    Ok(())
}
