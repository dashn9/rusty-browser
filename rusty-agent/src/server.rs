use std::sync::Arc;

use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tonic::{Request, Response, Status};
use tonic::transport::{Identity, Server, ServerTlsConfig};
use tokio_stream::wrappers::TcpListenerStream;
use tracing::info;

use rusty_proto::browser_agent_server::{BrowserAgent, BrowserAgentServer};
use rusty_proto::master_client::MasterClient;
use rusty_proto::{BrowserCommand, CommandResult, RegisterAgentRequest};

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
    native_tls: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let cert = std::fs::read_to_string("agent.crt")
        .map_err(|e| TlsError::CertRead(e.to_string()))?;
    let key = std::fs::read_to_string("agent.key")
        .map_err(|e| TlsError::KeyRead(e.to_string()))?;
    let tls = ServerTlsConfig::new()
        .identity(Identity::from_pem(&cert, &key));

    let listener = TcpListener::bind("0.0.0.0:0").await?;
    let grpc_port = listener.local_addr()?.port();
    let private_ip = local_ip();
    let public_ip = detect_public_ip().await.unwrap_or_else(|| private_ip.clone());

    info!("Browser agent {browser_id} listening on {public_ip}/{private_ip}:{grpc_port} (TLS)");

    let registration = RegisterAgentRequest {
        execution_id: execution_id.to_string(),
        browser_id: browser_id.to_string(),
        public_ip: public_ip.clone(),
        private_ip: private_ip.clone(),
        grpc_port: grpc_port as u32,
    };
    let browser_id_owned = browser_id.to_string();
    let (master, master_tls) = if native_tls {
        // Use system/native root CAs — lets the agent verify cert without bundled master.crt
        let tls = tonic::transport::ClientTlsConfig::new().with_native_roots();
        (master_url.to_string(), tls)
    } else {
        let cert = std::fs::read_to_string("master.crt")
            .map_err(|e| TlsError::CertRead(format!("master.crt: {e}")))?;
        let tls = tonic::transport::ClientTlsConfig::new()
            .ca_certificate(tonic::transport::Certificate::from_pem(&cert))
            .domain_name("rusty-master");
        (master_url.to_string(), tls)
    };

    tokio::spawn(async move {
        let channel = tonic::transport::Channel::from_shared(master)
            .expect("valid master URL")
            .tls_config(master_tls)
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

async fn detect_public_ip() -> Option<String> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut stream = tokio::net::TcpStream::connect("api.ipify.org:80").await.ok()?;
    stream.write_all(b"GET / HTTP/1.0\r\nHost: api.ipify.org\r\n\r\n").await.ok()?;
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).await.ok()?;
    let response = String::from_utf8(buf).ok()?;
    response.split("\r\n\r\n").nth(1).map(|s| s.trim().to_string())
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
