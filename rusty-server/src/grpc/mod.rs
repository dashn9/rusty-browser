use std::sync::Arc;

use tonic::{Request, Response, Status};
use tracing::{info, warn};

use rusty_common::state::{BrowserInfo, BrowserState};
use rusty_proto::master_server::Master;
use rusty_proto::{RegisterAgentRequest, RegisterResponse};

use crate::AppState;

pub struct MasterService {
    pub state: Arc<AppState>,
}

#[tonic::async_trait]
impl Master for MasterService {
    async fn register(
        &self,
        request: Request<RegisterAgentRequest>,
    ) -> Result<Response<RegisterResponse>, Status> {
        let req = request.into_inner();
        info!("Recieved registration request for execution={} browser={}", req.execution_id, req.browser_id);

        let info = BrowserInfo {
            browser_id: req.browser_id.clone(),
            execution_id: req.execution_id.clone(),
            public_ip: req.public_ip.clone(),
            private_ip: req.private_ip.clone(),
            grpc_port: req.grpc_port as u16,
            state: BrowserState::PartialReserved,
            contexts: vec![],
        };

        self.state.redis.upsert_browser(&info).await.map_err(|e| {
            warn!("Failed to register agent execution={} browser={}: {e}", req.execution_id, req.browser_id);
            Status::internal(e.to_string())
        })?;

        info!("Agent registered: browser={} execution={} public={}:{} private={}", req.browser_id, req.execution_id, req.public_ip, req.grpc_port, req.private_ip);

        Ok(Response::new(RegisterResponse { ok: true }))
    }
}
