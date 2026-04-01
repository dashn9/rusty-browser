use reqwest::Client;
use serde::Deserialize;

use crate::error::{FluxError, RustmaniError};
use crate::state::BrowserInfo;

#[derive(Clone)]
pub struct FluxClient {
    client: Client,
    url: String,
    token: String,
}

#[derive(Debug, Deserialize)]
struct FluxResponse {
    output: String,
    error: Option<String>,
}

impl FluxClient {
    pub fn new(url: &str, token: &str) -> Self {
        Self {
            client: Client::new(),
            url: url.trim_end_matches('/').to_string(),
            token: token.to_string(),
        }
    }

    pub async fn health(&self) -> Result<bool, RustmaniError> {
        let resp = self.client.get(format!("{}/health", self.url)).send().await
            .map_err(FluxError::Request)?;
        Ok(resp.status().is_success())
    }

    pub async fn initialize(&self) -> Result<(), RustmaniError> {
        let resp = self.client
            .post(format!("{}/initialize", self.url))
            .header("X-API-Key", &self.token)
            .send()
            .await
            .map_err(FluxError::Request)?;
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(FluxError::Http { status, body }.into());
        }
        Ok(())
    }

    pub async fn register_function(&self, yaml_body: &str) -> Result<(), RustmaniError> {
        let resp = self.client
            .put(format!("{}/functions", self.url))
            .header("X-API-Key", &self.token)
            .header("Content-Type", "application/yaml")
            .body(yaml_body.to_string())
            .send()
            .await
            .map_err(FluxError::Request)?;
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(FluxError::Http { status, body }.into());
        }
        Ok(())
    }

    /// Deploy a `.deb` package to Flux as a multipart form upload.
    ///
    /// `filename` becomes the `filename` attribute on the multipart part so
    /// Flux can identify the package by name.
    pub async fn deploy_function_multipart(
        &self,
        name: &str,
        filename: &str,
        zip_bytes: Vec<u8>,
    ) -> Result<(), RustmaniError> {
        let part = reqwest::multipart::Part::bytes(zip_bytes)
            .file_name(filename.to_string())
            .mime_str("application/zip")
            .map_err(|e| FluxError::Request(e))?;

        let form = reqwest::multipart::Form::new().part("file", part);

        let resp = self.client
            .put(format!("{}/deploy/{name}", self.url))
            .header("X-API-Key", &self.token)
            .multipart(form)
            .send()
            .await
            .map_err(FluxError::Request)?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(FluxError::Http { status, body }.into());
        }
        Ok(())
    }

    /// Spawns a browser agent via Flux. Returns BrowserInfo (browser_id, host, grpc_port)
    /// parsed from the function's stdout output.
    pub async fn execute_function(&self, name: &str, args: &[String]) -> Result<BrowserInfo, FluxError> {
        let resp = self.client
            .post(format!("{}/execute/{name}", self.url))
            .header("X-API-Key", &self.token)
            .json(&serde_json::json!({ "args": args }))
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(FluxError::Http { status: status.as_u16(), body });
        }

        let flux_resp: FluxResponse = resp.json().await
            .map_err(|e| FluxError::Parse(e.to_string()))?;

        if let Some(err) = flux_resp.error {
            if !err.is_empty() {
                return Err(FluxError::Execution(err));
            }
        }

        serde_json::from_str::<BrowserInfo>(&flux_resp.output)
            .map_err(|e| FluxError::Parse(format!("Failed to parse agent info: {e}")))
    }

    pub async fn terminate_all_nodes(&self) -> Result<(), RustmaniError> {
        let resp = self.client
            .delete(format!("{}/nodes", self.url))
            .header("X-API-Key", &self.token)
            .send()
            .await
            .map_err(FluxError::Request)?;
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(FluxError::Http { status, body }.into());
        }
        Ok(())
    }
}
