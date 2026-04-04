use serde::{Deserialize, Serialize};

use crate::error::ConfigError;

#[derive(Debug, Clone, Deserialize)]
pub struct RustmaniConfig {
    pub server: ServerConfig,
    pub redis: RedisConfig,
    pub ai: AIConfig,
    pub flux: FluxConfig,
    pub api_keys: Vec<String>,
    #[serde(default)]
    pub browser: BrowserConfig,
    /// Path to the agent-proxies.yaml file to bundle into the agent deployment.
    #[serde(default = "default_proxy_file")]
    pub proxy_file: String,
    /// Extra environment variables injected into the agent function spec.
    #[serde(default)]
    pub agent_env: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct BrowserConfig {
    #[serde(default)]
    pub chrome_config: Option<ChromeBrowserConfig>,
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

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ChromeBrowserConfig {
    #[serde(default)]
    pub driver_executable_path: Option<String>,
    #[serde(default)]
    pub host: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub driver_flags: Vec<String>,
    #[serde(default)]
    pub sandbox: bool,
    #[serde(default)]
    pub chrome_executable_path: Option<String>,
    #[serde(default)]
    pub user_data_dir: Option<String>,
    #[serde(default)]
    pub browser_flags: Vec<String>,
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
    #[serde(alias = "base_url")]
    pub url: String,
    #[serde(alias = "api_key")]
    pub token: String,
    #[serde(default = "default_function_name")]
    pub function_name: String,
    #[serde(default)]
    pub github_release_base_url: Option<String>,
}

fn default_proxy_file() -> String {
    "agent-proxies.yaml".to_string()
}

fn default_function_name() -> String {
    "rustmani-agent".to_string()
}

impl RustmaniConfig {
    pub fn load(path: &str) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| ConfigError::Read(e.to_string()))?;
        let content = substitute_env_vars(&content);
        yaml_serde::from_str(&content)
            .map_err(|e| ConfigError::Parse(e.to_string()))
    }
}

fn substitute_env_vars(input: &str) -> String {
    let mut result = input.to_string();
    while let Some(start) = result.find("${") {
        if let Some(end) = result[start..].find('}') {
            let var_name = &result[start + 2..start + end];
            let value = std::env::var(var_name).unwrap_or_default();
            result = format!(
                "{}{}{}",
                &result[..start],
                value,
                &result[start + end + 1..]
            );
        } else {
            break;
        }
    }
    result
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyList(Vec<GeoProxyEntry>);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeoProxyEntry {
    pub geo: String,
    #[serde(default)]
    pub proxies: Vec<String>,
}

impl ProxyList {
    pub fn load(path: &str) -> Option<Self> {
        let content = std::fs::read_to_string(path).ok()?;
        yaml_serde::from_str(&content).ok()
    }

    pub fn get_proxies_for_geo(&self, geo: Option<&str>) -> Vec<String> {
        let target_geo = geo.unwrap_or("").to_uppercase();

        for entry in &self.0 {
            if entry.geo.to_uppercase() == target_geo {
                return entry.proxies.clone();
            }
        }

        Vec::new()
    }

    pub fn get_all(&self) -> Vec<&str> {
        self.0.iter()
            .flat_map(|e| e.proxies.iter().map(String::as_str))
            .collect()
    }
}
