use std::sync::Arc;

use serde::Serialize;
use tracing::info;

use rustmani_common::error::RustmaniError;

use crate::AppState;

const BROWSER_CONFIG_ENV_VAR: &str = "RUSTMANI_BROWSER_CONFIG";
const PROXIES_FILENAME: &str = "agent-proxies.yaml";

#[derive(Serialize)]
struct Resources {
    cpu: u32,
    memory: u32,
}

#[derive(Serialize)]
struct FunctionSpec {
    name: String,
    handler: String,
    resources: Resources,
    timeout: u32,
    max_concurrency: u32,
    #[serde(rename = "max_concurrency_behaviour")]
    max_concurrency_behaviour: String,
    #[serde(rename = "resource_pressure_behavior")]
    resource_pressure_behavior: String,
    env: serde_yaml::Value,
}

pub struct InitializeService {
    state: Arc<AppState>,
}

impl InitializeService {
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }

    pub async fn run_initialization(&self) -> Result<(), RustmaniError> {
        let function_name = self.state.config.flux.function_name.clone();
        let flux = &self.state.flux;

        info!("Initializing Flux runtime…");
        flux.initialize().await?;
        info!("Flux initialized");

        info!("Registering function '{function_name}'…");
        let browser_config_json = self.get_browser_config_json();
        let function_yaml = build_function_yaml(&function_name, &browser_config_json, &self.state.config.agent_env)?;
        flux.register_function(&function_yaml).await?;
        info!("Function '{function_name}' registered");

        let version = "0.1.0";
        let filename = "rustmani-agent";
        info!("Downloading {filename}…");
        let agent_bytes = self.download_agent(version, filename).await?;
        info!("Downloaded {} byte(s)", agent_bytes.len());

        info!("Zipping {filename}…");
        let proxies_yaml = self.get_proxies_yaml();
        let zip_bytes = create_zip_with_proxies(filename, &agent_bytes, proxies_yaml)?;

        info!("Uploading '{filename}.zip' to Flux as function '{function_name}'…");
        flux.deploy_function_multipart(&function_name, &format!("{filename}.zip"), zip_bytes)
            .await?;
        info!("Agent '{function_name}' v{version} deployed");

        Ok(())
    }

    fn get_proxies_yaml(&self) -> Option<String> {
        let path = self.state.config.proxy_file.as_deref()?;
        std::fs::read_to_string(path).ok()
    }

    fn get_browser_config_json(&self) -> Option<String> {
        let chrome_config = self.state.config.browser.chrome_config.as_ref()?;
        serde_json::to_string(chrome_config).ok()
    }

    async fn download_agent(
        &self,
        version: &str,
        filename: &str,
    ) -> Result<Vec<u8>, RustmaniError> {
        let base = self
            .state
            .config
            .flux
            .github_release_base_url
            .as_deref()
            .unwrap_or("https://github.com/dashn9/rustmani/releases/download");

        let url = format!("{base}/v{version}/{filename}");
        info!("GET {url}");

        let client = reqwest::Client::new();
        let resp = client
            .get(&url)
            .send()
            .await
            .map_err(|e| RustmaniError::Internal(format!("Download failed: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(RustmaniError::Internal(format!(
                "Download returned HTTP {status}: {body}"
            )));
        }

        resp.bytes()
            .await
            .map(|b| b.to_vec())
            .map_err(|e| RustmaniError::Internal(format!("Failed to read download body: {e}")))
    }
}

fn build_function_yaml(
    name: &str,
    browser_config_json: &Option<String>,
    agent_env: &std::collections::HashMap<String, String>,
) -> Result<String, RustmaniError> {
    let mut env_vars = serde_yaml::Mapping::new();

    if let Some(config) = browser_config_json {
        env_vars.insert(
            serde_yaml::Value::String(BROWSER_CONFIG_ENV_VAR.to_string()),
            serde_yaml::Value::String(config.clone()),
        );
    }

    for (k, v) in agent_env {
        env_vars.insert(
            serde_yaml::Value::String(k.clone()),
            serde_yaml::Value::String(v.clone()),
        );
    }

    let spec = FunctionSpec {
        name: name.to_string(),
        handler: name.to_string(),
        resources: Resources {
            cpu: 1,
            memory: 2048,
        },
        timeout: 30,
        max_concurrency: 200,
        max_concurrency_behaviour: "wait".to_string(),
        resource_pressure_behavior: "wait".to_string(),
        env: serde_yaml::Value::Mapping(env_vars),
    };

    serde_yaml::to_string(&spec)
        .map_err(|e| RustmaniError::Internal(format!("YAML serialization failed: {e}")))
}

fn create_zip_with_proxies(
    filename: &str,
    agent_data: &[u8],
    proxies_yaml: Option<String>,
) -> Result<Vec<u8>, RustmaniError> {
    use std::io::{Cursor, Write};
    let cursor = Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(cursor);

    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o755);

    zip.start_file(filename, options)
        .map_err(|e| RustmaniError::Internal(format!("zip start_file: {}", e)))?;

    zip.write_all(agent_data)
        .map_err(|e| RustmaniError::Internal(format!("zip write_all: {}", e)))?;

    if let Some(proxies) = proxies_yaml {
        let file_options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o644);

        zip.start_file(PROXIES_FILENAME, file_options)
            .map_err(|e| RustmaniError::Internal(format!("zip start_file proxies: {}", e)))?;

        zip.write_all(proxies.as_bytes())
            .map_err(|e| RustmaniError::Internal(format!("zip write_all proxies: {}", e)))?;
    }

    zip.finish()
        .map(|c| c.into_inner())
        .map_err(|e| RustmaniError::Internal(format!("zip finish: {}", e)))
}
