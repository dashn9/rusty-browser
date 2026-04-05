use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use async_stream::stream;
use tokio_stream::Stream;
use tonic::{Request, Response, Status};
use tracing::{info, warn};

use rustmani_common::state::{BrowserInfo, BrowserState};
use rustmani_proto::master_server::Master;
use rustmani_proto::{ConnectEvent, RegisterAgentRequest};

use crate::AppState;

pub struct MasterService {
    pub state: Arc<AppState>,
}

#[tonic::async_trait]
impl Master for MasterService {
    type RegisterStream = Pin<Box<dyn Stream<Item = Result<ConnectEvent, Status>> + Send>>;

    async fn register(
        &self,
        request: Request<RegisterAgentRequest>,
    ) -> Result<Response<Self::RegisterStream>, Status> {
        let req = request.into_inner();

        let info = BrowserInfo {
            browser_id: req.browser_id.clone(),
            execution_id: req.execution_id.clone(),
            host: req.host.clone(),
            grpc_port: req.grpc_port as u16,
            state: BrowserState::Idle,
            contexts: vec![],
        };

        self.state.redis.upsert_browser(&info).await
            .map_err(|e| Status::internal(e.to_string()))?;

        info!("Agent connected: browser={} execution={} addr={}:{}", req.browser_id, req.execution_id, req.host, req.grpc_port);

        let execution_id = req.execution_id.clone();
        let state = self.state.clone();

        let output = stream! {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            loop {
                interval.tick().await;
                yield Ok(ConnectEvent { terminate: false });
            }
        };

        // Clean up browser record when the stream ends (agent disconnected).
        // Redis is keyed by execution_id; browser_id is stored metadata only.
        let cleanup = {
            let state = state.clone();
            let execution_id = execution_id.clone();
            async move {
                warn!("Agent disconnected: execution={execution_id}");
                if let Err(e) = state.redis.remove_browser(&execution_id).await {
                    warn!("Cleanup failed for execution {execution_id}: {e}");
                }
            }
        };

        let guarded = async_stream::stream! {
            tokio::pin!(output);
            while let Some(event) = {
                use tokio_stream::StreamExt;
                output.next().await
            } {
                yield event;
            }
            cleanup.await;
        };

        Ok(Response::new(Box::pin(guarded)))
    }
}
