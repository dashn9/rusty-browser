use serde::{Deserialize, Serialize};

use crate::error::ConfigError;

#[derive(Debug, Clone, Deserialize)]
pub struct RustyConfig {
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
    #[serde(default)]
    pub deployment: DeploymentConfig,
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
    /// Local port the gRPC server binds to. Omit to let the OS assign a free port.
    pub grpc_port: Option<u16>,
    /// The gRPC URL advertised to agents. Use this to set an ngrok tunnel or any
    /// externally reachable address. If unset, defaults to https://{public_ip}:{grpc_port}.
    pub grpc_server_url: Option<String>,
    /// Accept plain gRPC connections on the master server (no TLS). For development/local use only.
    #[serde(default)]
    pub insecure_grpc: bool,
}

fn default_http_port() -> u16 { 8080 }

#[derive(Debug, Clone, Deserialize)]
pub struct RedisConfig {
    pub url: String,
    #[serde(default = "default_key_prefix")]
    pub key_prefix: String,
}

fn default_key_prefix() -> String {
    "rusty:".to_string()
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
    #[serde(default = "default_quality")]
    pub quality: f32,
}

impl Default for ResolutionConfig {
    fn default() -> Self {
        Self {
            quality: default_quality(),
        }
    }
}

fn default_quality() -> f32 {
    0.85
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
    #[serde(default = "default_pending_timeout_secs")]
    pub pending_timeout_secs: u64,
    /// If set, spawn the agent as a local subprocess instead of calling Flux (dev/testing only).
    #[serde(default)]
    pub local_binary: Option<String>,
}

fn default_pending_timeout_secs() -> u64 { 10 }

fn default_proxy_file() -> String {
    "agent-proxies.yaml".to_string()
}

fn default_function_name() -> String {
    "rusty-agent".to_string()
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct DeploymentConfig {
    #[serde(default)]
    pub agent_os_target: AgentOsTarget,
}

#[derive(Debug, Clone, Deserialize, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AgentOsTarget {
    #[default]
    Linux,
    Windows,
}

impl AgentOsTarget {
    pub fn binary_name(&self) -> &'static str {
        match self {
            AgentOsTarget::Linux => "rusty-agent",
            AgentOsTarget::Windows => "rusty-agent.exe",
        }
    }
}

impl RustyConfig {
    pub fn load(path: &str) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| ConfigError::Read(e.to_string()))?;
        let content = substitute_env_vars(&content);
        let config: Self = yaml_serde::from_str(&content)
            .map_err(|e| ConfigError::Parse(e.to_string()))?;
        let q = config.ai.resolution.quality;
        if !(0.0..=1.0).contains(&q) {
            return Err(ConfigError::Parse(format!(
                "ai.resolution.quality must be between 0.0 and 1.0, got {q}"
            )));
        }
        Ok(config)
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
pub struct ProxyList(std::collections::HashMap<String, GeoProxyEntry>);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeoProxyEntry {
    #[serde(default)]
    pub proxies: Vec<String>,
}

impl ProxyList {
    pub fn load(path: &str) -> Option<Self> {
        let content = std::fs::read_to_string(path).ok()?;
        match yaml_serde::from_str(&content) {
            Ok(list) => Some(list),
            Err(e) => {
                tracing::warn!("Failed to parse {path}: {e}");
                None
            }
        }
    }

    pub fn get_proxies_for_geo(&self, geo: Option<&str>) -> Vec<String> {
        let target_geo = geo.unwrap_or("").to_uppercase();
        self.0.get(&target_geo).map(|e| e.proxies.clone()).unwrap_or_default()
    }

    pub fn get_all(&self) -> Vec<&str> {
        self.0.values()
            .flat_map(|e| e.proxies.iter().map(String::as_str))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_temp_config(content: &str) -> tempfile::NamedTempFile {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(content.as_bytes()).unwrap();
        f
    }

    fn minimal_yaml(extra: &str) -> String {
        format!(
            r#"
server:
  http_port: 8080
redis:
  url: "redis://localhost:6379"
ai:
  provider: openai
  api_key: "sk-test"
  model: "gpt-4o"
flux:
  url: "https://flux.example.com"
  token: "tok123"
api_keys:
  - "apikey1"
{extra}
"#
        )
    }

    // ---- RustyConfig::load ----

    #[test]
    fn load_minimal_valid_config() {
        let f = write_temp_config(&minimal_yaml(""));
        let cfg = RustyConfig::load(f.path().to_str().unwrap()).unwrap();
        assert_eq!(cfg.server.http_port, 8080);
        assert_eq!(cfg.redis.url, "redis://localhost:6379");
        assert_eq!(cfg.ai.provider, AIProviderKind::OpenAI);
        assert_eq!(cfg.ai.api_key, "sk-test");
        assert_eq!(cfg.ai.model, "gpt-4o");
        assert_eq!(cfg.flux.url, "https://flux.example.com");
        assert_eq!(cfg.flux.token, "tok123");
        assert_eq!(cfg.api_keys, vec!["apikey1"]);
    }

    #[test]
    fn load_applies_default_http_port() {
        let yaml = r#"
server: {}
redis:
  url: "redis://localhost:6379"
ai:
  provider: openai
  api_key: "k"
  model: "m"
flux:
  url: "https://x.com"
  token: "t"
api_keys: ["k1"]
"#;
        let f = write_temp_config(yaml);
        let cfg = RustyConfig::load(f.path().to_str().unwrap()).unwrap();
        assert_eq!(cfg.server.http_port, 8080);
    }

    #[test]
    fn load_applies_default_key_prefix() {
        let f = write_temp_config(&minimal_yaml(""));
        let cfg = RustyConfig::load(f.path().to_str().unwrap()).unwrap();
        assert_eq!(cfg.redis.key_prefix, "rusty:");
    }

    #[test]
    fn load_applies_default_function_name() {
        let f = write_temp_config(&minimal_yaml(""));
        let cfg = RustyConfig::load(f.path().to_str().unwrap()).unwrap();
        assert_eq!(cfg.flux.function_name, "rusty-agent");
    }

    #[test]
    fn load_applies_default_quality() {
        let f = write_temp_config(&minimal_yaml(""));
        let cfg = RustyConfig::load(f.path().to_str().unwrap()).unwrap();
        assert!((cfg.ai.resolution.quality - 0.85).abs() < f32::EPSILON);
    }

    #[test]
    fn load_applies_default_pending_timeout_secs() {
        let f = write_temp_config(&minimal_yaml(""));
        let cfg = RustyConfig::load(f.path().to_str().unwrap()).unwrap();
        assert_eq!(cfg.flux.pending_timeout_secs, 10);
    }

    #[test]
    fn load_custom_quality_valid() {
        let extra = "ai:\n  provider: openai\n  api_key: k\n  model: m\n  resolution:\n    quality: 0.5\n";
        let yaml = format!(
            r#"
server:
  http_port: 8080
redis:
  url: "redis://localhost:6379"
flux:
  url: "https://x.com"
  token: "t"
api_keys: ["k1"]
{extra}
"#
        );
        let f = write_temp_config(&yaml);
        let cfg = RustyConfig::load(f.path().to_str().unwrap()).unwrap();
        assert!((cfg.ai.resolution.quality - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn load_rejects_quality_above_one() {
        let yaml = r#"
server:
  http_port: 8080
redis:
  url: "redis://localhost:6379"
ai:
  provider: openai
  api_key: k
  model: m
  resolution:
    quality: 1.5
flux:
  url: "https://x.com"
  token: "t"
api_keys: ["k1"]
"#;
        let f = write_temp_config(yaml);
        let err = RustyConfig::load(f.path().to_str().unwrap()).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("quality"), "expected quality error, got: {msg}");
    }

    #[test]
    fn load_rejects_quality_below_zero() {
        let yaml = r#"
server:
  http_port: 8080
redis:
  url: "redis://localhost:6379"
ai:
  provider: openai
  api_key: k
  model: m
  resolution:
    quality: -0.1
flux:
  url: "https://x.com"
  token: "t"
api_keys: ["k1"]
"#;
        let f = write_temp_config(yaml);
        let err = RustyConfig::load(f.path().to_str().unwrap()).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("quality"), "expected quality error, got: {msg}");
    }

    #[test]
    fn load_accepts_quality_zero() {
        let yaml = r#"
server:
  http_port: 8080
redis:
  url: "redis://localhost:6379"
ai:
  provider: openai
  api_key: k
  model: m
  resolution:
    quality: 0.0
flux:
  url: "https://x.com"
  token: "t"
api_keys: ["k1"]
"#;
        let f = write_temp_config(yaml);
        let cfg = RustyConfig::load(f.path().to_str().unwrap()).unwrap();
        assert!((cfg.ai.resolution.quality - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn load_accepts_quality_one() {
        let yaml = r#"
server:
  http_port: 8080
redis:
  url: "redis://localhost:6379"
ai:
  provider: openai
  api_key: k
  model: m
  resolution:
    quality: 1.0
flux:
  url: "https://x.com"
  token: "t"
api_keys: ["k1"]
"#;
        let f = write_temp_config(yaml);
        let cfg = RustyConfig::load(f.path().to_str().unwrap()).unwrap();
        assert!((cfg.ai.resolution.quality - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn load_missing_file_returns_read_error() {
        let err = RustyConfig::load("/nonexistent/path/config.yaml").unwrap_err();
        assert!(matches!(err, ConfigError::Read(_)));
    }

    #[test]
    fn load_invalid_yaml_returns_parse_error() {
        let f = write_temp_config("not: valid: yaml: ][");
        let err = RustyConfig::load(f.path().to_str().unwrap()).unwrap_err();
        assert!(matches!(err, ConfigError::Parse(_)));
    }

    #[test]
    fn load_openrouter_provider() {
        let yaml = r#"
server:
  http_port: 8080
redis:
  url: "redis://localhost:6379"
ai:
  provider: openrouter
  api_key: "or-key"
  model: "mistral"
flux:
  url: "https://x.com"
  token: "t"
api_keys: ["k1"]
"#;
        let f = write_temp_config(yaml);
        let cfg = RustyConfig::load(f.path().to_str().unwrap()).unwrap();
        assert_eq!(cfg.ai.provider, AIProviderKind::OpenRouter);
    }

    #[test]
    fn load_flux_alias_fields() {
        let yaml = r#"
server:
  http_port: 8080
redis:
  url: "redis://localhost:6379"
ai:
  provider: openai
  api_key: k
  model: m
flux:
  base_url: "https://alias.example.com"
  api_key: "alias-tok"
api_keys: ["k1"]
"#;
        let f = write_temp_config(yaml);
        let cfg = RustyConfig::load(f.path().to_str().unwrap()).unwrap();
        assert_eq!(cfg.flux.url, "https://alias.example.com");
        assert_eq!(cfg.flux.token, "alias-tok");
    }

    #[test]
    fn load_local_binary_field() {
        let extra = "  local_binary: \"/usr/local/bin/rusty-agent\"";
        let yaml = format!(
            r#"
server:
  http_port: 8080
redis:
  url: "redis://localhost:6379"
ai:
  provider: openai
  api_key: k
  model: m
flux:
  url: "https://x.com"
  token: "t"
{extra}
api_keys: ["k1"]
"#
        );
        let f = write_temp_config(&yaml);
        let cfg = RustyConfig::load(f.path().to_str().unwrap()).unwrap();
        assert_eq!(cfg.flux.local_binary.as_deref(), Some("/usr/local/bin/rusty-agent"));
    }

    // ---- AgentOsTarget ----

    #[test]
    fn agent_os_target_linux_binary_name() {
        assert_eq!(AgentOsTarget::Linux.binary_name(), "rusty-agent");
    }

    #[test]
    fn agent_os_target_windows_binary_name() {
        assert_eq!(AgentOsTarget::Windows.binary_name(), "rusty-agent.exe");
    }

    #[test]
    fn agent_os_target_default_is_linux() {
        assert_eq!(AgentOsTarget::default(), AgentOsTarget::Linux);
    }

    // ---- ProxyList ----

    fn proxy_list_from_yaml(yaml: &str) -> ProxyList {
        yaml_serde::from_str(yaml).unwrap()
    }

    #[test]
    fn proxy_list_get_proxies_for_geo_match() {
        let list: ProxyList = proxy_list_from_yaml(
            r#"
US:
  proxies:
    - "proxy1.us:3128"
    - "proxy2.us:3128"
GB:
  proxies:
    - "proxy1.gb:3128"
"#,
        );
        let proxies = list.get_proxies_for_geo(Some("US"));
        assert_eq!(proxies.len(), 2);
        assert!(proxies.contains(&"proxy1.us:3128".to_string()));
    }

    #[test]
    fn proxy_list_get_proxies_for_geo_case_insensitive() {
        let list: ProxyList = proxy_list_from_yaml(
            r#"
US:
  proxies:
    - "proxy.us:3128"
"#,
        );
        let proxies = list.get_proxies_for_geo(Some("us"));
        assert_eq!(proxies.len(), 1);
    }

    #[test]
    fn proxy_list_get_proxies_for_geo_unknown_returns_empty() {
        let list: ProxyList = proxy_list_from_yaml(
            r#"
US:
  proxies:
    - "proxy.us:3128"
"#,
        );
        let proxies = list.get_proxies_for_geo(Some("DE"));
        assert!(proxies.is_empty());
    }

    #[test]
    fn proxy_list_get_proxies_for_geo_none_key_looks_up_empty_string() {
        let list: ProxyList = proxy_list_from_yaml(
            r#"
US:
  proxies:
    - "proxy.us:3128"
"#,
        );
        // None maps to "" which won't match "US"
        let proxies = list.get_proxies_for_geo(None);
        assert!(proxies.is_empty());
    }

    #[test]
    fn proxy_list_get_all_collects_across_geos() {
        let list: ProxyList = proxy_list_from_yaml(
            r#"
US:
  proxies:
    - "proxy.us:3128"
GB:
  proxies:
    - "proxy.gb:3128"
    - "proxy2.gb:3128"
"#,
        );
        let all = list.get_all();
        assert_eq!(all.len(), 3);
        assert!(all.contains(&"proxy.us:3128"));
        assert!(all.contains(&"proxy.gb:3128"));
        assert!(all.contains(&"proxy2.gb:3128"));
    }

    #[test]
    fn proxy_list_get_all_empty() {
        let list: ProxyList = proxy_list_from_yaml(
            r#"
US:
  proxies: []
"#,
        );
        assert!(list.get_all().is_empty());
    }

    #[test]
    fn proxy_list_load_missing_file_returns_none() {
        let result = ProxyList::load("/nonexistent/path/proxies.yaml");
        assert!(result.is_none());
    }

    #[test]
    fn proxy_list_load_invalid_yaml_returns_none() {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(b"not: valid: yaml: ][").unwrap();
        let result = ProxyList::load(f.path().to_str().unwrap());
        assert!(result.is_none());
    }

    #[test]
    fn proxy_list_load_valid_file() {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(b"US:\n  proxies:\n    - proxy.us:3128\n").unwrap();
        let list = ProxyList::load(f.path().to_str().unwrap()).unwrap();
        assert_eq!(list.get_proxies_for_geo(Some("US")).len(), 1);
    }

    // ---- ResolutionConfig default ----

    #[test]
    fn resolution_config_default_quality() {
        let r = ResolutionConfig::default();
        assert!((r.quality - 0.85).abs() < f32::EPSILON);
    }
}
