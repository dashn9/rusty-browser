use std::sync::Arc;

use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tonic::{Request, Response, Status};
use tonic::transport::{Identity, Server, ServerTlsConfig};
use tokio_stream::wrappers::TcpListenerStream;
use tracing::info;

use rustmani_proto::browser_agent_server::{BrowserAgent, BrowserAgentServer};
use rustmani_proto::master_client::MasterClient;
use rustmani_proto::{BrowserCommand, CommandResult, RegisterAgentRequest};

use crate::browser::ManagedBrowser;
use crate::error::{GrpcError, TlsError};
use crate::executor;

struct BrowserAgentService {
    browser: Arc<Mutex<ManagedBrowser>>,
}

#[tonic::async_trait]
impl BrowserAgent for BrowserAgentService {
    async fn execute(
        &self,
        request: Request<BrowserCommand>,
    ) -> Result<Response<CommandResult>, Status> {
        let cmd = request.into_inner();
        let mut browser = self.browser.lock().await;
        let result = executor::execute(&mut *browser, cmd).await.unwrap_or_else(|e| CommandResult {
            success: false,
            error_message: e.to_string(),
            result: String::new(),
        });
        Ok(Response::new(result))
    }
}

pub async fn serve(
    browser: ManagedBrowser,
    browser_id: &str,
    execution_id: &str,
    master_url: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let cert = std::fs::read_to_string("agent.crt")
        .map_err(|e| TlsError::CertRead(e.to_string()))?;
    let key = std::fs::read_to_string("agent.key")
        .map_err(|e| TlsError::KeyRead(e.to_string()))?;
    let tls = ServerTlsConfig::new()
        .identity(Identity::from_pem(&cert, &key));

    let listener = TcpListener::bind("0.0.0.0:0").await?;
    let grpc_port = listener.local_addr()?.port();
    let host = std::env::var("RUSTMANI_AGENT_HOST").unwrap_or_else(|_| local_ip());

    info!("Browser agent {browser_id} listening on {host}:{grpc_port} (TLS)");

    // Register with master — stream stays open for liveness
    let registration = RegisterAgentRequest {
        execution_id: execution_id.to_string(),
        browser_id: browser_id.to_string(),
        host: host.clone(),
        grpc_port: grpc_port as u32,
    };
    let master = master_url.to_string();
    let browser_id_owned = browser_id.to_string();
    let master_cert = std::fs::read_to_string("master.crt")
        .map_err(|e| TlsError::CertRead(format!("master.crt: {e}")))?;

    tokio::spawn(async move {
        let tls = tonic::transport::ClientTlsConfig::new()
            .ca_certificate(tonic::transport::Certificate::from_pem(&master_cert))
            .domain_name("rustmani-master");

        let channel = tonic::transport::Channel::from_shared(master)
            .expect("valid master URL")
            .tls_config(tls)
            .expect("master TLS config")
            .connect_lazy();

        let mut client = MasterClient::new(channel);
        info!("Connecting to master, registering browser={browser_id_owned}");
        match client.register(Request::new(registration)).await {
            Ok(_) => info!("Registered with master"),
            Err(e) => tracing::error!("Failed to register with master: {e}"),
        }
    });

    Server::builder()
        .tls_config(tls)
        .map_err(|e| TlsError::Config(e.to_string()))?
        .add_service(BrowserAgentServer::new(BrowserAgentService {
            browser: Arc::new(Mutex::new(browser)),
        }))
        .serve_with_incoming(TcpListenerStream::new(listener))
        .await
        .map_err(|e| GrpcError::Serve(e.to_string()))?;

    Ok(())
}

/// Determine the outbound IP by opening a UDP socket toward a public address.
/// No packets are sent — this just reveals which local interface would be used.
fn local_ip() -> String {
    std::net::UdpSocket::bind("0.0.0.0:0")
        .and_then(|s| { s.connect("8.8.8.8:80")?; s.local_addr() })
        .map(|a| a.ip().to_string())
        .unwrap_or_else(|_| {
            tracing::warn!("Could not determine local IP, falling back to 127.0.0.1");
            "127.0.0.1".to_string()
        })
}
