use std::io::{Cursor, Write};
use std::sync::Arc;

use rcgen::generate_simple_self_signed;
use serde::Serialize;
use tracing::info;

use crate::http::error::AppError;
use crate::AppState;

const BROWSER_CONFIG_ENV_VAR: &str = "RUSTY_BROWSER_CONFIG";
const TLS_CERT_FILENAME: &str = "agent.crt";
const TLS_KEY_FILENAME: &str = "agent.key";

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

    pub async fn run_initialization(&self) -> Result<(), AppError> {
        let function_name = self.state.config.flux.function_name.clone();
        let flux = &self.state.flux;

        info!("Initializing Flux runtime…");
        flux.initialize().await?;
        info!("Flux initialized");

        let version = "0.1.0";
        let filename = self.state.config.deployment.agent_os_target.binary_name();

        info!("Registering function '{function_name}'…");
        let browser_config_json = self.get_browser_config_json();
        let function_yaml = build_function_yaml(&function_name, filename, &browser_config_json, &self.state.config.agent_env)?;
        flux.register_function(&function_yaml).await?;
        info!("Function '{function_name}' registered");
        info!("Downloading {filename}…");
        let agent_bytes = self.download_agent(version, filename).await?;
        info!("Downloaded {} byte(s)", agent_bytes.len());

        info!("Generating TLS cert…");
        let (cert_pem, key_pem) = generate_tls_cert()?;
        self.state.redis.set_tls_cert(&cert_pem).await?;
        info!("TLS cert stored");

        info!("Zipping {filename}…");
        let proxy_file = &self.state.config.proxy_file;
        let tls_cert_tmp = write_temp(TLS_CERT_FILENAME, cert_pem.as_bytes())?;
        let tls_key_tmp  = write_temp(TLS_KEY_FILENAME,  key_pem.as_bytes())?;

        let (master_cert_pem, _) = self.state.redis.get_master_tls_cert().await?
            .ok_or_else(|| AppError::Internal("Master TLS cert not found — server may not have started correctly".into()))?;
        let master_cert_tmp = write_temp("master.crt", master_cert_pem.as_bytes())?;

        let zip_bytes = create_zip(filename, &agent_bytes, &[
            proxy_file.as_str(),
            &tls_cert_tmp,
            &tls_key_tmp,
            &master_cert_tmp,
        ])?;

        info!("Uploading '{filename}.zip' to Flux as function '{function_name}'…");
        flux.deploy_function_multipart(&function_name, &format!("{filename}.zip"), zip_bytes).await?;
        info!("Agent '{function_name}' v{version} deployed");

        Ok(())
    }

    fn get_browser_config_json(&self) -> Option<String> {
        let chrome_config = self.state.config.browser.chrome_config.as_ref()?;
        serde_json::to_string(chrome_config).ok()
    }

    async fn download_agent(&self, version: &str, filename: &str) -> Result<Vec<u8>, AppError> {
        let base = self.state.config.flux.github_release_base_url
            .as_deref()
            .unwrap_or("https://github.com/dashn9/rusty-browser/releases/download");

        let url = format!("{base}/v{version}/{filename}");
        info!("GET {url}");

        let resp = reqwest::Client::new()
            .get(&url)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Download failed: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::Internal(format!("Download returned HTTP {status}: {body}")));
        }

        resp.bytes()
            .await
            .map(|b| b.to_vec())
            .map_err(|e| AppError::Internal(format!("Failed to read download body: {e}")))
    }
}

fn build_function_yaml(
    name: &str,
    handler: &str,
    browser_config_json: &Option<String>,
    agent_env: &std::collections::HashMap<String, String>,
) -> Result<String, AppError> {
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
        handler: handler.to_string(),
        resources: Resources { cpu: 1, memory: 2048 },
        timeout: 0,
        max_concurrency: 200,
        max_concurrency_behaviour: "wait".to_string(),
        resource_pressure_behavior: "wait".to_string(),
        env: serde_yaml::Value::Mapping(env_vars),
    };

    serde_yaml::to_string(&spec)
        .map_err(|e| AppError::Internal(format!("YAML serialization failed: {e}")))
}

fn create_zip(binary_name: &str, binary_data: &[u8], extra_files: &[&str]) -> Result<Vec<u8>, AppError> {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));

    let exe_opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o755);
    let file_opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o644);

    zip.start_file(binary_name, exe_opts)
        .map_err(|e| AppError::Internal(format!("zip: {e}")))?;
    zip.write_all(binary_data)
        .map_err(|e| AppError::Internal(format!("zip: {e}")))?;

    for path in extra_files {
        match std::fs::read(path) {
            Ok(data) => {
                let name = std::path::Path::new(path)
                    .file_name().and_then(|n| n.to_str()).unwrap_or(path);
                zip.start_file(name, file_opts)
                    .map_err(|e| AppError::Internal(format!("zip: {e}")))?;
                zip.write_all(&data)
                    .map_err(|e| AppError::Internal(format!("zip: {e}")))?;
                tracing::info!("Bundled {path} as {name}");
            }
            Err(e) => tracing::warn!("Skipping {path}: {e}"),
        }
    }

    zip.finish()
        .map(|c| c.into_inner())
        .map_err(|e| AppError::Internal(format!("zip finish: {e}")))
}

fn generate_tls_cert() -> Result<(String, String), AppError> {
    let cert = generate_simple_self_signed(vec!["rusty-agent".to_string()])
        .map_err(|e| AppError::Internal(format!("TLS cert generation failed: {e}")))?;
    let cert_pem = cert.cert.pem();
    let key_pem = cert.key_pair.serialize_pem();
    Ok((cert_pem, key_pem))
}

fn write_temp(name: &str, data: &[u8]) -> Result<String, AppError> {
    let path = std::env::temp_dir().join(name);
    std::fs::write(&path, data)
        .map_err(|e| AppError::Internal(format!("write temp {name}: {e}")))?;
    path.to_str()
        .map(|s| s.to_string())
        .ok_or_else(|| AppError::Internal(format!("invalid temp path for {name}")))
}
