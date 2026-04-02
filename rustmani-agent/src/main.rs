use tracing::info;

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

    let browser_id = std::env::var("RUSTMANI_BROWSER_ID")
        .unwrap_or_else(|_| uuid::Uuid::new_v4().to_string());

    let identity_json = std::env::var("RUSTMANI_IDENTITY_JSON").ok();

    info!("Starting rustmani-agent {browser_id}");

    let browser = browser::ManagedBrowser::launch(identity_json.as_deref()).await?;
    info!("Browser launched");

    server::serve(browser, &browser_id).await?;

    Ok(())
}
