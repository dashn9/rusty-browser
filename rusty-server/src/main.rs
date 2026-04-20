use std::sync::Arc;

use anyhow::Result;
use tokio::net::TcpListener;
use tracing::info;

mod grpc;
mod http;
mod services;

use rusty_common::config::RustyConfig;
use rusty_common::flux::FluxClient;
use rusty_common::redis_store::RedisStore;
use rusty_common::ui_map::UiNode;
use rusty_common::util::{detect_public_ip, free_port};
use rusty_proto::master_server::MasterServer;
use tower_http::trace::TraceLayer;

pub struct AppState {
    pub config: RustyConfig,
    pub redis: RedisStore,
    pub flux: FluxClient,
    pub ai_provider: Box<dyn rusty_common::ai::AIProvider>,
    pub public_ip: String,
    pub instruct_running: std::sync::atomic::AtomicBool,
    pub ui_map_cache: std::sync::Mutex<std::collections::HashMap<String, Vec<UiNode>>>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Ensure ring is used as the rustls crypto provider — reqwest and tonic both
    // pull in rustls, and without an explicit selection the process panics.
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install ring crypto provider");

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".parse().unwrap())
        )
        .init();

    let config_path = std::env::var("RUSTY_CONFIG")
        .unwrap_or_else(|_| "rusty.yaml".to_string());

    let mut config = RustyConfig::load(&config_path)?;
    let grpc_port = config.server.grpc_port.unwrap_or_else(free_port);
    config.server.grpc_port = Some(grpc_port);
    info!("Loaded configuration from {config_path}");

    let public_ip = detect_public_ip().await
        .expect("failed to detect public IP — check network connectivity");
    info!("Detected public IP: {public_ip}");

    let redis = RedisStore::new(&config.redis.url, &config.redis.key_prefix).await?;
    info!("Connected to Redis");

    let flux = FluxClient::new(&config.flux.url, &config.flux.token);
    let ai_provider = rusty_common::ai::create_provider(&config.ai);

    let state = Arc::new(AppState {
        config: config.clone(),
        redis,
        flux,
        ai_provider,
        public_ip,
        instruct_running: std::sync::atomic::AtomicBool::new(false),
        ui_map_cache: std::sync::Mutex::new(std::collections::HashMap::new()),
    });

    let http_port = config.server.http_port;
    let grpc_port = config.server.grpc_port.expect("grpc_port resolved at startup");

    // Cancel and clean up pending agents that never registered within the timeout
    tokio::spawn({
        let redis = state.redis.clone();
        let flux = state.flux.clone();
        let timeout = config.flux.pending_timeout_secs;
        async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(timeout));
            loop {
                interval.tick().await;
                match redis.list_stale_agents(timeout).await {
                    Ok(ids) if !ids.is_empty() => {
                        for id in ids {
                            tracing::warn!("Pending agent timed out, cancelling: {id}");
                            let _ = flux.cancel_execution(&id).await;
                            let _ = redis.remove_browser(&id).await;
                        }
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
            let cert = rcgen::generate_simple_self_signed(vec!["rusty-master".to_string()])?;
            let cert_pem = cert.cert.pem();
            let key_pem = cert.key_pair.serialize_pem();
            state.redis.set_master_tls_cert(&cert_pem, &key_pem).await?;
            info!("Master TLS cert stored");
            (cert_pem, key_pem)
        }
    };

    // TODO: Due to rusty hiding behind a reverse proxy that uses it's own tls to establish the connection
    // WE can opt for the payload itself to be encrypted bi server and agent while tls does it's own thing if we have have or delegate control of it
    // gRPC server — agents connect here to register
    let grpc_state = state.clone();
    let insecure_grpc = config.server.insecure_grpc;
    tokio::spawn(async move {
        let addr = format!("0.0.0.0:{}", grpc_port).parse().expect("valid grpc addr");
        let mut builder = tonic::transport::Server::builder();
        if insecure_grpc {
            info!("Master gRPC listening on {addr} (insecure, no TLS)");
        } else {
            info!("Master gRPC listening on {addr} (TLS)");
            let identity = tonic::transport::Identity::from_pem(&master_cert_pem, &master_key_pem);
            builder = builder
                .tls_config(tonic::transport::ServerTlsConfig::new().identity(identity))
                .expect("master TLS config failed");
        }
        builder
            .layer(TraceLayer::new_for_grpc())
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
