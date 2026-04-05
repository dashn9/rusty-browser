use std::sync::Arc;

use anyhow::Result;
use tokio::net::TcpListener;
use tracing::info;

mod grpc;
mod http;
mod services;

use rustmani_common::config::RustmaniConfig;
use rustmani_common::flux::FluxClient;
use rustmani_common::redis_store::RedisStore;
use rustmani_proto::master_server::MasterServer;

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

    let http_port = config.server.http_port;
    let grpc_port = config.server.grpc_port;

    // Warn about agents that were spawned but never registered
    tokio::spawn({
        let redis = state.redis.clone();
        async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
            loop {
                interval.tick().await;
                match redis.list_stale_agents(60).await {
                    Ok(ids) if !ids.is_empty() => {
                        tracing::warn!("Executions with no agent registration after 60s: {:?}", ids);
                    }
                    Err(e) => tracing::warn!("Stale agent check failed: {e}"),
                    _ => {}
                }
            }
        }
    });

    // Ensure master TLS cert exists — generate once, reuse across restarts
    let (master_cert_pem, master_key_pem) = match state.redis.get_master_tls_cert().await? {
        Some(pair) => {
            info!("Loaded master TLS cert from Redis");
            pair
        }
        None => {
            info!("Generating master TLS cert…");
            let cert = rcgen::generate_simple_self_signed(vec!["rustmani-master".to_string()])?;
            let cert_pem = cert.cert.pem();
            let key_pem = cert.key_pair.serialize_pem();
            state.redis.set_master_tls_cert(&cert_pem, &key_pem).await?;
            info!("Master TLS cert stored");
            (cert_pem, key_pem)
        }
    };

    // gRPC server — agents connect here to register
    let grpc_state = state.clone();
    tokio::spawn(async move {
        let addr = format!("0.0.0.0:{grpc_port}").parse().expect("valid grpc addr");
        info!("Master gRPC listening on {addr} (TLS)");
        let identity = tonic::transport::Identity::from_pem(&master_cert_pem, &master_key_pem);
        tonic::transport::Server::builder()
            .tls_config(tonic::transport::ServerTlsConfig::new().identity(identity))
            .expect("master TLS config failed")
            .add_service(MasterServer::new(grpc::MasterService { state: grpc_state }))
            .serve(addr)
            .await
            .expect("gRPC server failed");
    });

    // HTTP server
    let app = http::router(state);
    let listener = TcpListener::bind(format!("0.0.0.0:{http_port}")).await?;
    info!("HTTP server listening on 0.0.0.0:{http_port}");
    axum::serve(listener, app).await?;

    Ok(())
}
