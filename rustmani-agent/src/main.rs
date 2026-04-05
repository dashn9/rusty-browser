use tracing::info;
use uuid::Uuid;

mod browser;
mod error;
mod executor;
mod server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Pin ring as the rustls crypto provider before any TLS handshake.
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install ring crypto provider");

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "rustmani_agent=info".parse().unwrap()),
        )
        .init();

    let execution_id = std::env::var("FLUX_EXECUTION_ID")
        .expect("FLUX_EXECUTION_ID must be set by Flux");

    // master_url is passed as the first positional arg by the server at spawn time
    let master_url = std::env::args().nth(1)
        .expect("master_url must be passed as the first argument");

    let browser_id = Uuid::new_v4().to_string();
    let browser_config = browser::ChromeBrowserLaunchConfig::from_env().unwrap_or_default();

    info!("Starting rustmani-agent browser={browser_id} execution={execution_id}");

    let browser = browser::ManagedBrowser::launch(browser_config).await?;

    server::serve(browser, &browser_id, &execution_id, &master_url).await?;

    Ok(())
}
