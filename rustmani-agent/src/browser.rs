use rand::seq::SliceRandom;
use rustenium::browsers::BidiBrowser;
use rustenium::browsers::chrome::browser::ChromeConfig;
use rustenium_identity::{IdentityConfig, IdentitySession, preset::random};
use rustmani_common::config::ProxyList;
use serde::Deserialize;
use uuid::Uuid;

use crate::error::BrowserError;

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ChromeBrowserLaunchConfig {
    pub driver_executable_path: Option<String>,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub driver_flags: Vec<String>,
    pub sandbox: bool,
    pub chrome_executable_path: Option<String>,
    pub user_data_dir: Option<String>,
    pub browser_flags: Vec<String>,
}

impl ChromeBrowserLaunchConfig {
    pub fn from_env() -> Option<Self> {
        std::env::var("RUSTMANI_BROWSER_CONFIG")
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
    }
}

impl From<ChromeBrowserLaunchConfig> for ChromeConfig {
    fn from(cfg: ChromeBrowserLaunchConfig) -> Self {
        let mut chrome_cfg: ChromeConfig = ChromeConfig::default();
        
        if let Some(path) = cfg.driver_executable_path {
            chrome_cfg.driver_executable_path = path;
        }
        if let Some(host) = cfg.host {
            chrome_cfg.host = Some(host);
        }
        if let Some(port) = cfg.port {
            chrome_cfg.port = Some(port);
        }
        if !cfg.driver_flags.is_empty() {
            let leaked: Vec<&'static str> = cfg.driver_flags.into_iter()
                .map(|s| -> &'static str { Box::leak(s.into_boxed_str()) as &str })
                .collect();
            chrome_cfg.driver_flags = leaked;
        }
        chrome_cfg.sandbox = cfg.sandbox;
        if let Some(path) = cfg.chrome_executable_path {
            chrome_cfg.chrome_executable_path = Some(path);
        }
        if let Some(dir) = cfg.user_data_dir {
            chrome_cfg.user_data_dir = Some(dir);
        }
        if !cfg.browser_flags.is_empty() {
            chrome_cfg.browser_flags = Some(cfg.browser_flags);
        }
        
        chrome_cfg
    }
}

pub struct ManagedBrowser {
    id: Uuid,
    session: IdentitySession,
}

impl ManagedBrowser {
    pub async fn launch(
        browser_config: ChromeBrowserLaunchConfig,
    ) -> Result<Self, BrowserError> {
        let mut identity = random();
        identity.proxy = Self::select_proxy(&identity.geo);

        let config = IdentityConfig::new(identity, browser_config.into());
        let mut session = IdentitySession::launch(config)
            .await
            .map_err(|e| BrowserError::Launch(e.to_string()))?;
        session.browser_mut().connect_bidi().await;
        let id = Uuid::new_v4();
        tracing::info!("[ManagedBrowser] launched {id}");
        Ok(Self { id, session })
    }

    fn select_proxy(geo: &rustenium_identity::IdentityCountryGeo) -> Option<String> {
        let list = match ProxyList::load("agent-proxies.yaml") {
            Some(l) => l,
            None => {
                tracing::warn!("[ManagedBrowser] agent-proxies.yaml not found, skipping proxy");
                return None;
            }
        };
        let mut rng = rand::thread_rng();
        let by_geo = list.get_proxies_for_geo(Some(geo.as_str()));
        let proxy = if !by_geo.is_empty() {
            by_geo.choose(&mut rng).cloned()
        } else {
            tracing::warn!("[ManagedBrowser] no proxies for geo={}, falling back to all", geo.as_str());
            list.get_all().choose(&mut rng).map(|s| s.to_string())
        };
        tracing::info!("[ManagedBrowser] proxy selected: {:?} (geo={})", proxy, geo.as_str());
        proxy
    }

    pub async fn navigate(&mut self, url: &str, _wait_until: &str) -> Result<(), BrowserError> {
        tracing::info!("[ManagedBrowser] {} Navigate to: {}", self.id, url);
        self.session.browser_mut().navigate(url).await
            .map(|_| ()).map_err(|e| BrowserError::Navigate(e.to_string()))
    }

    pub async fn screenshot(&self) -> Result<Vec<u8>, BrowserError> {
        tracing::info!(r"[ManagedBrowser] screenshot ");
        Ok(Vec::new())
    }

    pub async fn click(&self, x: f32, y: f32, human: bool) -> Result<(), BrowserError> {
        tracing::info!("[ManagedBrowser] {} Click ({}, {}) human={}", self.id, x, y, human);
        Ok(())
    }

    pub async fn type_text(&self, text: &str, _selector: &str) -> Result<(), BrowserError> {
        tracing::info!("[ManagedBrowser] {} Type: {}", self.id, text);
        Ok(())
    }

    pub async fn mouse_move(&self, x: f32, y: f32, _steps: u32) -> Result<(), BrowserError> {
        tracing::info!("[ManagedBrowser] {} Mouse move ({}, {})", self.id, x, y);
        Ok(())
    }

    pub async fn human_mouse_move(&self, x: f32, y: f32) -> Result<(), BrowserError> {
        tracing::info!("[ManagedBrowser] {} Human mouse move ({}, {})", self.id, x, y);
        Ok(())
    }

    pub async fn create_context(&self, url: &str) -> Result<String, BrowserError> {
        tracing::info!("[ManagedBrowser] {} Create context url={}", self.id, url);
        Ok(Uuid::new_v4().to_string())
    }

    pub async fn close_context(&self, context_id: &str) -> Result<(), BrowserError> {
        tracing::info!("[ManagedBrowser] {} Close context {}", self.id, context_id);
        Ok(())
    }

    pub async fn eval_js(&self, script: &str) -> Result<String, BrowserError> {
        tracing::info!(r"[ManagedBrowser] eval JS ");
        let _ = script;
        Ok(String::new())
    }

    pub async fn find_node(&self, selector: &str) -> Result<bool, BrowserError> {
        tracing::info!("[ManagedBrowser] {} Find node: {}", self.id, selector);
        Ok(false)
    }

    pub async fn wait_for_node(&self, selector: &str, _timeout_ms: u64) -> Result<bool, BrowserError> {
        tracing::info!("[ManagedBrowser] {} Wait for node: {}", self.id, selector);
        Ok(false)
    }

    pub async fn emulate_device(&self, width: u32, height: u32, scale: f32, mobile: bool) -> Result<(), BrowserError> {
        tracing::info!("[ManagedBrowser] {} Emulate {}x{} scale={} mobile={}", self.id, width, height, scale, mobile);
        Ok(())
    }

    pub async fn scroll(&self, delta_x: f32, delta_y: f32) -> Result<(), BrowserError> {
        tracing::info!("[ManagedBrowser] {} Scroll dx={} dy={}", self.id, delta_x, delta_y);
        Ok(())
    }

    pub async fn close(&self) -> Result<(), BrowserError> {
        tracing::info!("closing ");
        Ok(())
    }
}
