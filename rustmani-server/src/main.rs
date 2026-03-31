use std::sync::Arc;

use anyhow::Result;
use tokio::net::TcpListener;
use tracing::info;

mod http;
mod services;

use rustmani_common::config::RustmaniConfig;
use rustmani_common::flux::FluxClient;
use rustmani_common::redis_store::RedisStore;

pub struct AppState {
    pub config: RustmaniConfig,
    pub redis: RedisStore,
    pub flux: FluxClient,
    pub ai_provider: Box<dyn rustmani_common::ai::AIProvider>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "rustmani=info,tower_http=info".parse().unwrap()),
        )
        .init();

    let config_path = std::env::var("RUSTMANI_CONFIG")
        .unwrap_or_else(|_| "rustmani.yaml".to_string());

    let config = RustmaniConfig::load(&config_path)?;
    info!("Loaded configuration from {config_path}");

    let redis = RedisStore::new(&config.redis.url, &config.redis.key_prefix).await?;
    info!("Connected to Redis");

    let flux = FluxClient::new(&config.flux.url, &config.flux.token);
    let ai_provider = rustmani_common::ai::create_provider(&config.ai);

    let state = Arc::new(AppState {
        config: config.clone(),
        redis,
        flux,
        ai_provider,
    });

    // Start HTTP server
    let http_port = config.server.http_port;
    let app = http::router(state);
    let listener = TcpListener::bind(format!("0.0.0.0:{http_port}")).await?;
    info!("HTTP server listening on 0.0.0.0:{http_port}");
    axum::serve(listener, app).await?;

    Ok(())
}
