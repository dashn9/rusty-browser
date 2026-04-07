use rand::Rng;
use rand::seq::SliceRandom;
use rustenium::browsers::chrome::browser::ChromeConfig;
use rustenium::browsers::{
    BidiBrowser, BrowserScreenshotOptionsBuilder, WaitForNodesOptionsBuilder,
};
use rustenium::domain::context::BrowsingContext;
use rustenium::input::{KeyboardTypeOptionsBuilder, MouseClickOptions, MouseMoveOptions, Point};
use rustenium::input::Mouse;
use rustenium::nodes::Node;
use rustenium_bidi_definitions::browsing_context::commands::CaptureScreenshotOrigin;
use rustenium_identity::preset::get_by_id;
use rustenium_identity::{IdentityConfig, IdentitySession};
use rustenium_macros::css;
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
        let mut chrome_cfg = ChromeConfig::default();
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
            let leaked: Vec<&'static str> = cfg
                .driver_flags
                .into_iter()
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
    contexts: std::collections::HashMap<String, BrowsingContext>,
}

impl ManagedBrowser {
    pub async fn launch(browser_config: ChromeBrowserLaunchConfig) -> Result<Self, BrowserError> {
        let mut identity = get_by_id(1).unwrap();
        identity.proxy = Self::select_proxy(&identity.geo);

        let config = IdentityConfig::new(identity, browser_config.into());
        let mut session = IdentitySession::launch(config)
            .await
            .map_err(|e| BrowserError::Launch(e.to_string()))?;
        session.browser_mut().connect_bidi().await;
        let id = Uuid::new_v4();
        tracing::info!("[ManagedBrowser] launched {id}");
        Ok(Self {
            id,
            session,
            contexts: std::collections::HashMap::new(),
        })
    }

    fn select_proxy(geo: &rustenium_identity::IdentityCountryGeo) -> Option<String> {
        if !std::path::Path::new("agent-proxies.yaml").exists() {
            tracing::warn!("[ManagedBrowser] agent-proxies.yaml not found, skipping proxy");
            return None;
        }
        let list = ProxyList::load("agent-proxies.yaml")?;
        let mut rng = rand::thread_rng();
        let by_geo = list.get_proxies_for_geo(Some(geo.as_str()));
        let proxy = if !by_geo.is_empty() {
            by_geo.choose(&mut rng).cloned()
        } else {
            tracing::warn!(
                "[ManagedBrowser] no proxies for geo={}, falling back to all",
                geo.as_str()
            );
            list.get_all().choose(&mut rng).map(|s| s.to_string())
        };
        tracing::info!(
            "[ManagedBrowser] proxy selected: {:?} (geo={})",
            proxy,
            geo.as_str()
        );
        proxy
    }

    fn active_context(&self) -> String {
        self.session
            .browser()
            .get_active_context_id()
            .expect("No active context id for browser")
            .inner()
            .to_owned()
    }

    pub async fn navigate(&mut self, url: &str, _wait_until: &str) -> Result<(), BrowserError> {
        tracing::info!("[ManagedBrowser] {} Navigate to: {}", self.id, url);
        self.session
            .browser_mut()
            .navigate(url)
            .await
            .map(|_| ())
            .map_err(|e| BrowserError::Navigate(e.to_string()))
    }

    pub async fn screenshot(&mut self) -> Result<String, BrowserError> {
        let mut opts = BrowserScreenshotOptionsBuilder::default();
        opts = opts.origin(CaptureScreenshotOrigin::Document);
        self.session
            .browser_mut()
            .screenshot_with_options(opts.build())
            .await
            .map_err(|e| BrowserError::Screenshot(e.to_string()))
    }

    pub async fn click(&mut self, x: f32, y: f32, human: bool) -> Result<(), BrowserError> {
        tracing::info!(
            "[ManagedBrowser] {} Click ({}, {}) human={}",
            self.id,
            x,
            y,
            human
        );
        let ctx = self.active_context().into();
        let point = Some(Point {
            x: x as f64,
            y: y as f64,
        });
        if human {
            self.session
                .browser_mut()
                .human_mouse()
                .click(point, &ctx, MouseClickOptions::default())
                .await
                .map_err(|e| BrowserError::Click(e.to_string()))
        } else {
            self.session
                .browser_mut()
                .mouse()
                .click(point, &ctx, MouseClickOptions::default())
                .await
                .map_err(|e| BrowserError::Click(e.to_string()))
        }
    }

    pub async fn node_click(&mut self, selector: &str, human: bool) -> Result<(), BrowserError> {
        tracing::info!(
            "[ManagedBrowser] {} NodeClick selector={} human={}",
            self.id,
            selector,
            human
        );
        let mut node = self
            .session
            .browser_mut()
            .find_node(css!(selector))
            .await
            .map_err(|e| BrowserError::Click(e.to_string()))?
            .ok_or_else(|| BrowserError::Click(format!("Node not found: {selector}")))?;
        let ctx = self.active_context().into();
        let position = node.get_position().await.ok_or_else(|| {
            BrowserError::Click(format!("Could not get position for node: {selector}"))
        })?;
        if position.width == 0.0 || position.height == 0.0 {
            return Err(BrowserError::Click(format!("Node has zero dimensions: {selector}")));
        }
        let point = random_point(position.x, position.y, position.width, position.height);
        if human {
            self.session
                .browser_mut()
                .human_mouse()
                .click(Some(point), &ctx, MouseClickOptions::default())
                .await
                .map_err(|e| BrowserError::Click(e.to_string()))
        } else {
            self.session
                .browser_mut()
                .mouse()
                .click(Some(point), &ctx, MouseClickOptions::default())
                .await
                .map_err(|e| BrowserError::Click(e.to_string()))
        }
    }

    pub async fn type_text(
        &mut self,
        text: String,
        selector: Option<String>,
    ) -> Result<(), BrowserError> {
        tracing::info!("[ManagedBrowser] {} Type: {}", self.id, text);
        if let Some(sel) = selector {
            let mut node = self
                .session
                .browser_mut()
                .find_node(css!(sel))
                .await
                .map_err(|e| BrowserError::TypeText(e.to_string()))?
                .ok_or_else(|| BrowserError::TypeText("Node not found".into()))?;
            return node
                .type_text(text)
                .await
                .map_err(|e| BrowserError::TypeText(e.to_string()));
        }
        let ctx = self.active_context().into();
        let opts = KeyboardTypeOptionsBuilder::default()
            .delay(60, 140)
            .gap_multiplier(1.2)
            .build();
        self.session
            .browser()
            .keyboard()
            .type_text(text.as_str(), &ctx, Some(opts))
            .await
            .map_err(|e| BrowserError::TypeText(e.to_string()))
    }

    pub async fn mouse_move(&self, x: f32, y: f32, _steps: u32) -> Result<(), BrowserError> {
        let ctx = self.active_context().into();
        self.session
            .browser()
            .mouse()
            .move_to(
                Point {
                    x: x as f64,
                    y: y as f64,
                },
                &ctx,
                MouseMoveOptions::default(),
            )
            .await
            .map_err(|e| BrowserError::Action(e.to_string()))
    }

    pub async fn human_mouse_move(&self, x: f32, y: f32) -> Result<(), BrowserError> {
        let ctx = self.active_context().into();
        self.session
            .browser()
            .human_mouse()
            .move_to(
                Point {
                    x: x as f64,
                    y: y as f64,
                },
                &ctx,
                MouseMoveOptions::default(),
            )
            .await
            .map_err(|e| BrowserError::Action(e.to_string()))
    }

    pub async fn scroll_by(&self, y: i32, _human: bool) -> Result<(), BrowserError> {
        let ctx = self.active_context().into();
        self.session
            .browser()
            .human_mouse()
            .scroll(y, 0, &ctx)
            .await
            .map_err(|e| BrowserError::Action(e.to_string()))
    }

    pub async fn scroll_to(
        &mut self,
        selector: &str,
        human: bool,
        to: u32,
    ) -> Result<(), BrowserError> {
        let mut node = self
            .session
            .browser_mut()
            .find_node(css!(selector))
            .await
            .map_err(|e| BrowserError::Action(e.to_string()))?
            .ok_or_else(|| BrowserError::Action(format!("Node not found: {selector}")))?;
        let position = node.get_position().await.ok_or_else(|| {
            BrowserError::Action(format!("Could not get position for: {selector}"))
        })?;
        // 0 = align top of element to viewport top, >0 (100) = align bottom to viewport bottom
        let target_y = if to == 0 {
            position.y as i32
        } else {
            (position.y + position.height) as i32
        };
        if human {
            let ctx = self.active_context().into();
            self.session
                .browser()
                .human_mouse()
                .scroll(target_y, 0, &ctx)
                .await
                .map_err(|e| BrowserError::Action(e.to_string()))
        } else {
            node.scroll_into_view()
                .await
                .map_err(|e| BrowserError::Action(e.to_string()))
        }
    }

    pub async fn create_context(&mut self, _url: &str) -> Result<String, BrowserError> {
        let browsing_ctx = self
            .session
            .browser_mut()
            .create_context(false)
            .await
            .map_err(|e| BrowserError::Context(e.to_string()))?;
        let id: String = browsing_ctx.id().inner().to_owned();
        self.contexts.insert(id.clone(), browsing_ctx);
        Ok(id)
    }

    pub async fn close_context(&mut self, context_id: &str) -> Result<(), BrowserError> {
        let browsing_ctx = self.contexts.remove(context_id)
            .ok_or_else(|| BrowserError::Context(format!("Context not found: {context_id}")))?;
        if let Err(e) = self.session.browser_mut().close_context(browsing_ctx.clone()).await {
            self.contexts.insert(context_id.to_string(), browsing_ctx);
            return Err(BrowserError::Context(e.to_string()));
        }
        Ok(())
    }

    pub async fn find_node(&mut self, selector: &str) -> Result<bool, BrowserError> {
        self.session
            .browser_mut()
            .find_node(css!(selector))
            .await
            .map(|n| n.is_some())
            .map_err(|e| BrowserError::Action(e.to_string()))
    }

    pub async fn wait_for_node(
        &mut self,
        selector: &str,
        timeout_ms: u64,
    ) -> Result<bool, BrowserError> {
        self.session
            .browser_mut()
            .wait_for_node_with_options(
                css!(selector),
                WaitForNodesOptionsBuilder::default()
                    .timeout_ms(timeout_ms)
                    .build(),
            )
            .await
            .map(|n| n.is_some())
            .map_err(|e| BrowserError::Action(e.to_string()))
    }

    pub async fn fetch_html(&mut self, selector: Option<&str>) -> Result<String, BrowserError> {
        let selector = selector.unwrap_or("html");
        let node = self
            .session
            .browser_mut()
            .find_node(css!(selector))
            .await
            .map_err(|e| BrowserError::Action(e.to_string()))?
            .ok_or_else(|| BrowserError::Action(format!("Node not found: {selector}")))?;
        return Ok(node.get_inner_html().await);
    }

    pub async fn fetch_text(&mut self, selector: &str) -> Result<String, BrowserError> {
        let node = self
            .session
            .browser_mut()
            .find_node(css!(selector))
            .await
            .map_err(|e| BrowserError::Action(e.to_string()))?
            .ok_or_else(|| BrowserError::Action(format!("Node not found: {selector}")))?;
        Ok(node.get_inner_text()
            .await)
    }

    pub async fn eval_js(&mut self, script: &str) -> Result<String, BrowserError> {
        self.session
            .browser_mut()
            .evaluate_script(script.to_string(), false)
            .await
            .map(|v| format!("{:?}", v))
            .map_err(|e| BrowserError::Action(format!("{:?}", e)))
    }

    pub async fn close(self) -> bool {
        self.session.close().await
    }
}

fn random_point(x: f64, y: f64, width: f64, height: f64) -> Point {
    let mut rng = rand::thread_rng();
    Point {
        x: x as f64 + rng.gen_range(0.0..width as f64),
        y: y as f64 + rng.gen_range(0.0..height as f64),
    }
}
