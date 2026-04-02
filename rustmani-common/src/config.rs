use serde::Deserialize;

use crate::error::RustmaniError;

#[derive(Debug, Clone, Deserialize)]
pub struct RustmaniConfig {
    pub server: ServerConfig,
    pub redis: RedisConfig,
    pub ai: AIConfig,
    pub flux: FluxConfig,
    pub api_keys: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_http_port")]
    pub http_port: u16,
}

fn default_http_port() -> u16 {
    8080
}

#[derive(Debug, Clone, Deserialize)]
pub struct RedisConfig {
    pub url: String,
    #[serde(default = "default_key_prefix")]
    pub key_prefix: String,
}

fn default_key_prefix() -> String {
    "rustmani:".to_string()
}

#[derive(Debug, Clone, Deserialize)]
pub struct AIConfig {
    pub provider: AIProviderKind,
    pub api_key: String,
    pub model: String,
    pub base_url: Option<String>,
    #[serde(default)]
    pub resolution: ResolutionConfig,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AIProviderKind {
    OpenAI,
    OpenRouter,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResolutionConfig {
    #[serde(default = "default_max_width")]
    pub max_width: u32,
    #[serde(default = "default_quality")]
    pub quality: u32,
    #[serde(default = "default_format")]
    pub format: String,
}

impl Default for ResolutionConfig {
    fn default() -> Self {
        Self {
            max_width: default_max_width(),
            quality: default_quality(),
            format: default_format(),
        }
    }
}

fn default_max_width() -> u32 {
    1280
}

fn default_quality() -> u32 {
    85
}

fn default_format() -> String {
    "jpeg".to_string()
}

#[derive(Debug, Clone, Deserialize)]
pub struct FluxConfig {
    /// Accepts both `url` and `base_url` in YAML.
    #[serde(alias = "base_url")]
    pub url: String,
    /// Accepts both `token` and `api_key` in YAML.
    #[serde(alias = "api_key")]
    pub token: String,
    /// Name of the Flux function to invoke. Read from FLUX_FUNCTION_NAME env var.
    #[serde(default = "default_function_name")]
    pub function_name: String,
    /// Base URL for GitHub Releases used to download the agent .deb.
    /// e.g. "https://github.com/wraithbytes/rustmani/releases/download"
    /// Falls back to the default repo URL when absent.
    #[serde(default)]
    pub github_release_base_url: Option<String>,
}

fn default_function_name() -> String {
    std::env::var("FLUX_FUNCTION_NAME").unwrap_or_else(|_| "rustmani-agent".to_string())
}

impl RustmaniConfig {
    pub fn load(path: &str) -> Result<Self, RustmaniError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| RustmaniError::Config(format!("Failed to read config: {e}")))?;

        // Substitute environment variables: ${VAR_NAME}
        let content = substitute_env_vars(&content);

        yaml_serde::from_str(&content)
            .map_err(|e| RustmaniError::Config(format!("Failed to parse config: {e}")))
    }
}

fn substitute_env_vars(input: &str) -> String {
    let mut result = input.to_string();
    while let Some(start) = result.find("${") {
        if let Some(end) = result[start..].find('}') {
            let var_name = &result[start + 2..start + end];
            let value = std::env::var(var_name).unwrap_or_default();
            result = format!("{}{}{}", &result[..start], value, &result[start + end + 1..]);
        } else {
            break;
        }
    }
    result
}
