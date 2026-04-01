use std::sync::Arc;

use tracing::info;

use rustmani_common::error::RustmaniError;

use crate::AppState;

/// Orchestrates the full cluster bootstrap sequence:
///   1. Initialize the Flux runtime.
///   2. Register or update the function definition.
///   3. Download the matching agent `.deb`.
///   4. Upload the `.deb` to Flux (multipart).
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

        // ── 1. Initialize Flux ─────────────────────────────────────────────
        info!("Initializing Flux runtime…");
        flux.initialize().await?;
        info!("Flux initialized");

        // ── 2. Register function definition ───────────────────────────────
        info!("Registering function '{function_name}'…");
        let function_yaml = build_function_yaml(&function_name);
        flux.register_function(&function_yaml).await?;
        info!("Function '{function_name}' registered");

        // ── 3. Download agent .deb ─────────────────────────────────────────
        let version = env!("CARGO_PKG_VERSION");
        let filename = format!("rustmani-agent_{version}_amd64.deb");
        info!("Downloading {filename}…");
        let deb_bytes = self.download_agent_deb(version, &filename).await?;
        info!("Downloaded {} byte(s)", deb_bytes.len());

        // ── 4. Zip the .deb ───────────────────────────────────────────────
        info!("Zipping {filename}…");
        let zip_bytes = create_zip(&filename, &deb_bytes)?;

        // ── 5. Deploy zip to Flux (multipart) ────────────────────────────
        info!("Uploading '{filename}.zip' to Flux as function '{function_name}'…");
        flux.deploy_function_multipart(&function_name, &format!("{filename}.zip"), zip_bytes)
            .await?;
        info!("Agent '{function_name}' v{version} deployed");

        Ok(())
    }

    async fn download_agent_deb(
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
            .unwrap_or("https://github.com/wraithbytes/rustmani/releases/download");

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

/// Build the YAML function definition sent to Flux's `PUT /functions` endpoint.
fn build_function_yaml(name: &str) -> String {
    format!(
        "name: {name}\n\
         handler: {name}\n\
         resources:\n\
           cpu: 1\n\
           memory: 2548\n\
         timeout: 30\n\
         max_concurrency: 100\n\
         max_concurrency_behaviour: wait\n\
         resource_pressure_behavior: wait\n\
         env:\n"
    )
}

fn create_zip(filename: &str, data: &[u8]) -> Result<Vec<u8>, RustmaniError> {
    use std::io::{Cursor, Write};
    let cursor = Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(cursor);

    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o644);

    zip.start_file(filename, options)
        .map_err(|e| RustmaniError::Internal(format!("zip start_file: {}", e)))?;

    zip.write_all(data)
        .map_err(|e| RustmaniError::Internal(format!("zip write_all: {}", e)))?;

    zip.finish()
        .map(|c| c.into_inner())
        .map_err(|e| RustmaniError::Internal(format!("zip finish: {}", e)))
}
