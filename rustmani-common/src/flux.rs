use reqwest::Client;
use serde::Deserialize;

use crate::error::FluxError;

#[derive(Debug, Deserialize)]
struct SpawnResponse {
    execution_id: String,
}

#[derive(Clone)]
pub struct FluxClient {
    client: Client,
    url: String,
    token: String,
}

impl FluxClient {
    pub fn new(url: &str, token: &str) -> Self {
        Self {
            client: Client::new(),
            url: url.trim_end_matches('/').to_string(),
            token: token.to_string(),
        }
    }

    pub async fn health(&self) -> Result<bool, FluxError> {
        let resp = self.client.get(format!("{}/health", self.url)).send().await?;
        Ok(resp.status().is_success())
    }

    pub async fn initialize(&self) -> Result<(), FluxError> {
        let resp = self.client
            .post(format!("{}/initialize", self.url))
            .header("X-API-Key", &self.token)
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(FluxError::Http { status, body });
        }
        Ok(())
    }

    pub async fn register_function(&self, yaml_body: &str) -> Result<(), FluxError> {
        let resp = self.client
            .put(format!("{}/functions", self.url))
            .header("X-API-Key", &self.token)
            .header("Content-Type", "application/yaml")
            .body(yaml_body.to_string())
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(FluxError::Http { status, body });
        }
        Ok(())
    }

    pub async fn deploy_function_multipart(
        &self,
        name: &str,
        filename: &str,
        zip_bytes: Vec<u8>,
    ) -> Result<(), FluxError> {
        let part = reqwest::multipart::Part::bytes(zip_bytes)
            .file_name(filename.to_string())
            .mime_str("application/zip")
            .map_err(FluxError::Request)?;

        let form = reqwest::multipart::Form::new().part("file", part);

        let resp = self.client
            .put(format!("{}/deploy/{name}", self.url))
            .header("X-API-Key", &self.token)
            .multipart(form)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(FluxError::Http { status, body });
        }
        Ok(())
    }

    /// Spawns a browser agent via Flux asynchronously. Returns the execution_id assigned
    /// by Flux, which the agent will echo back on registration so the server can map it
    /// to the correct browser_id. `args[0]` must be the master gRPC URL.
    pub async fn spawn_agent(&self, name: &str, args: &[String]) -> Result<String, FluxError> {
        let resp = self.client
            .post(format!("{}/execute/{name}/async", self.url))
            .header("X-API-Key", &self.token)
            .json(&serde_json::json!({ "args": args }))
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(FluxError::Http { status, body });
        }

        resp.json::<SpawnResponse>()
            .await
            .map(|r| r.execution_id)
            .map_err(|e| FluxError::Parse(format!("Failed to parse execution_id: {e}")))
    }

    /// Fetches the stdout/stderr log for a specific execution.
    pub async fn get_execution_logs(&self, execution_id: &str) -> Result<String, FluxError> {
        let resp = self.client
            .get(format!("{}/executions/{execution_id}/logs", self.url))
            .header("X-API-Key", &self.token)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(FluxError::Http { status, body });
        }

        resp.text().await.map_err(|e| FluxError::Parse(e.to_string()))
    }

    pub async fn terminate_all_nodes(&self) -> Result<(), FluxError> {
        let resp = self.client
            .delete(format!("{}/nodes", self.url))
            .header("X-API-Key", &self.token)
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(FluxError::Http { status, body });
        }
        Ok(())
    }
}
