use std::sync::Arc;

use anyhow::Result;
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tonic::{Request, Response, Status};
use tonic::transport::{Identity, Server, ServerTlsConfig};
use tokio_stream::wrappers::TcpListenerStream;

use rustmani_proto::browser_agent_server::{BrowserAgent, BrowserAgentServer};
use rustmani_proto::{BrowserCommand, CommandResult};

use crate::browser::ManagedBrowser;
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
        let browser = self.browser.lock().await;
        let result = executor::execute(&browser, cmd).await.unwrap_or_else(|e| CommandResult {
            success: false,
            error_message: e.to_string(),
            screenshot: None,
        });
        Ok(Response::new(result))
    }
}

pub async fn serve(browser: ManagedBrowser, browser_id: &str) -> Result<()> {
    let cert = std::fs::read_to_string("tls/agent.crt")?;
    let key = std::fs::read_to_string("tls/agent.key")?;
    let tls = ServerTlsConfig::new().identity(Identity::from_pem(&cert, &key));

    // Port 0 — OS assigns a free port, safe for multiple agents on the same node
    let listener = TcpListener::bind("0.0.0.0:0").await?;
    let grpc_port = listener.local_addr()?.port();
    let host = std::env::var("RUSTMANI_AGENT_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());

    // Print connection info for Flux to capture and return to the Master
    println!("{}", serde_json::json!({
        "browser_id": browser_id,
        "host": host,
        "grpc_port": grpc_port,
    }));

    tracing::info!("Browser agent {browser_id} listening on {host}:{grpc_port} (TLS)");

    Server::builder()
        .tls_config(tls)?
        .add_service(BrowserAgentServer::new(BrowserAgentService {
            browser: Arc::new(Mutex::new(browser)),
        }))
        .serve_with_incoming(TcpListenerStream::new(listener))
        .await?;

    Ok(())
}
