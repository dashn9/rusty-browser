use std::time::Duration;

use rand::Rng;
use rand::seq::SliceRandom;
use rustenium::browsers::ChromeTab;
use rustenium::browsers::cdp_browser::CdpBrowser;
use rustenium::browsers::cdp_browser::{
    BrowserScreenshotOptionsBuilder, FetchNodeOptions, Selector,
};
use rustenium::browsers::chrome::browser::ChromeConfig;
use rustenium::domain::cdp::page::Page;
use rustenium::domain::context::BrowsingContext;
use rustenium::input::Mouse;
use rustenium::input::{DelayRange, MouseClickOptions, MouseMoveOptions, Point};
use rustenium::nodes::{AXNode, Node};
use rustenium_bidi_definitions::browsing_context::types::CreateType;
use rustenium_cdp_definitions::browser_protocol::dom::types::{BackendNodeId, NodeId};
use rustenium_cdp_definitions::browser_protocol::page::commands::CaptureScreenshotFormat;
use rustenium_identity::preset::get_by_id;
use rustenium_identity::{IdentityConfig, IdentitySession};
use rusty_common::config::ProxyList;
use rusty_common::ui_map::UiNode;
use serde::Deserialize;
use uuid::Uuid;

use crate::error::BrowserError;

const CURSOR_SCRIPT: &str = concat!(
    r#"() => {
  const cur = document.createElement('img');
  cur.style.cssText = 'position:fixed;pointer-events:none;z-index:2147483647;transform:translate(-50%,-50%);transition:transform .1s;';
  cur.src = 'data:image/png;base64,"#,
    "iVBORw0KGgoAAAANSUhEUgAAABgAAAAYCAMAAADXqc3KAAAAJFBMVEXc3NylpaWrq6tRUVE8PDz8/Pzq6urU1NStra2IiIhYWFgbGxuSgdLUAAAADHRSTlMBS323+v////////7FbleZAAAAxUlEQVR42n1SUW7DUAx6SQ3YcP/7Tu2WdFOa8WsZY2Ad2CtJal9/8UizpUbnsd7YQk8CJGNle7MMIbwgcA6+rVBCV+xUQ4X63skUGcOk6ZA1ed0VxahAm7ACUU8FKSBCgAQIFKCy1t7DtjMNCT2xm9P7KgZFoAMAaYCFoFYmiIkTdJDJigDTOf6IaUD/DfoTVWcVbo7vPfolt2O3nnJXfH3Q+bEEkS+WrEzpNLEC1eRquw7bb4I6o1Ud0ZayfSgDzzLc1ucL/24MxntkAwMAAAAASUVORK5CYII=",
    r#"';
  document.documentElement.appendChild(cur);
  document.addEventListener('mousemove', e => { cur.style.left = e.clientX + 'px'; cur.style.top = e.clientY + 'px'; });
  document.addEventListener('touchmove', e => { const t = e.touches[0]; cur.style.left = t.clientX + 'px'; cur.style.top = t.clientY + 'px'; });
  const press = () => cur.style.transform = 'translate(-50%,-50%) scale(0.6)';
  const release = () => cur.style.transform = 'translate(-50%,-50%) scale(1)';
  document.addEventListener('mousedown', press); document.addEventListener('mouseup', release);
  document.addEventListener('touchstart', press); document.addEventListener('touchend', release);
}"#
);

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
        std::env::var("RUSTY_BROWSER_CONFIG")
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
    contexts: std::collections::HashMap<String, ChromeTab>,
}

impl ManagedBrowser {
    pub async fn launch(browser_config: ChromeBrowserLaunchConfig) -> Result<Self, BrowserError> {
        let mut identity = get_by_id(1).unwrap();
        identity.proxy = Self::select_proxy(&identity.geo);

        let config = IdentityConfig::new(identity, browser_config.into());
        let mut session = IdentitySession::launch(config)
            .await
            .map_err(|e| BrowserError::Launch(e.to_string()))?;
        let _ = session
            .browser_mut()
            .add_preload_script(format!("{CURSOR_SCRIPT}"))
            .await;
        let id = Uuid::new_v4();
        tracing::info!("launched {id}");
        Ok(Self {
            id,
            session,
            contexts: std::collections::HashMap::new(),
        })
    }

    fn select_proxy(geo: &rustenium_identity::IdentityCountryGeo) -> Option<String> {
        if !std::path::Path::new("agent-proxies.yaml").exists() {
            tracing::warn!("agent-proxies.yaml not found, skipping proxy");
            return None;
        }
        let list = ProxyList::load("agent-proxies.yaml")?;
        let mut rng = rand::thread_rng();
        let by_geo = list.get_proxies_for_geo(Some(geo.as_str()));
        let proxy = if !by_geo.is_empty() {
            by_geo.choose(&mut rng).cloned()
        } else {
            tracing::warn!("no proxies for geo={}, falling back to all", geo.as_str());
            list.get_all().choose(&mut rng).map(|s| s.to_string())
        };
        tracing::info!("proxy selected: {:?} (geo={})", proxy, geo.as_str());
        proxy
    }

    pub async fn navigate(&mut self, url: &str, _wait_until: &str) -> Result<(), BrowserError> {
        tracing::info!("{} Navigate to: {}", self.id, url);
        self.session
            .browser_mut()
            .navigate(url)
            .await
            .map(|_| ())
            .map_err(|e| BrowserError::Navigate(e.to_string()))?;
        tokio::time::sleep(Duration::from_secs(4)).await;
        let _ = self
            .session
            .browser_mut()
            .evaluate_script(format!("({CURSOR_SCRIPT})()"), false)
            .await;
        Ok(())
    }

    pub async fn screenshot(
        &mut self,
        quality: f32,
        _format: &str,
    ) -> Result<String, BrowserError> {
        let mut opts = BrowserScreenshotOptionsBuilder::default();
        opts = opts.full(true);
        // ignoring format for now
        opts = opts
            .format(CaptureScreenshotFormat::Jpeg)
            .quality(quality as f64);
        self.session
            .browser_mut()
            .screenshot_with_options(opts.build())
            .await
            .map_err(|e| BrowserError::Screenshot(e.to_string()))
    }

    pub async fn click(&mut self, x: f32, y: f32, human: bool) -> Result<(), BrowserError> {
        tracing::info!("{} Click ({}, {}) human={}", self.id, x, y, human);
        let point = Some(Point {
            x: x as f64,
            y: y as f64,
        });
        if human {
            let dud_ctx = BrowsingContext::from_id(String::new(), CreateType::Tab);
            self.session
                .browser_mut()
                .human_mouse()
                .click(point, dud_ctx.id(), MouseClickOptions::default())
                .await
                .map_err(|e| BrowserError::Click(e.to_string()))
        } else {
            self.session
                .browser_mut()
                .mouse()
                .click(point, MouseClickOptions::default())
                .await
                .map_err(|e| BrowserError::Click(e.to_string()))
        }
    }

    pub async fn node_click(&mut self, node_id: i64, human: bool) -> Result<(), BrowserError> {
        tracing::info!("{} NodeClick node_id={} human={}", self.id, node_id, human);
        // TODO: fetch_node is called twice (once here inside scroll_to, once below) — could be unified
        let _ = self.scroll_to(node_id, human).await;
        let mut node = self
            .session
            .browser_mut()
            .fetch_node(FetchNodeOptions::default().backend_node_id(BackendNodeId::new(node_id)))
            .await
            .map_err(|e| BrowserError::Click(e.to_string()))?;

        // let ctx = self.active_context().into();
        let ctx = BrowsingContext::from_id(String::new(), CreateType::Tab);
        let position = node.get_position().await.ok_or_else(|| {
            BrowserError::Click(format!("Could not get position for node: {node_id}"))
        })?;
        if position.width == 0.0 || position.height == 0.0 {
            return Err(BrowserError::Click(format!(
                "Node has zero dimensions: {node_id}"
            )));
        }
        let point = random_point(position.x, position.y, position.width, position.height);
        if human {
            self.session
                .browser_mut()
                .human_mouse()
                .click(Some(point), ctx.id(), MouseClickOptions::default())
                .await
                .map_err(|e| BrowserError::Click(e.to_string()))
        } else {
            self.session
                .browser_mut()
                .mouse()
                .click(Some(point), MouseClickOptions::default())
                .await
                .map_err(|e| BrowserError::Click(e.to_string()))
        }
    }

    pub async fn type_text(
        &mut self,
        text: String,
        node_id: Option<i64>,
    ) -> Result<(), BrowserError> {
        tracing::info!("{} Type: {}", self.id, text);
        if let Some(id) = node_id {
            let mut node = self
                .session
                .browser_mut()
                .fetch_node(FetchNodeOptions::default().node_id(NodeId::new(id)))
                .await
                .map_err(|e| BrowserError::TypeText(e.to_string()))?;
            return node
                .type_text(text)
                .await
                .map_err(|e| BrowserError::TypeText(e.to_string()));
        }
        // let ctx = self.active_context().into();
        // let opts = KeyboardTypeOptionsBuilder::default()
        //     .delay(60, 140)
        //     .gap_multiplier(1.2)
        //     .build();
        self.session
            .browser()
            .keyboard()
            .type_text(text.as_str(), 300)
            .await
            .map_err(|e| BrowserError::TypeText(e.to_string()))
    }

    pub async fn send_key(&self, key: &String, delay_ms: u64) -> Result<(), BrowserError> {
        self.session
            .browser()
            .keyboard()
            .press(key, delay_ms)
            .await
            .map_err(|e| BrowserError::Action(e.to_string()))
    }

    pub async fn hold_key(&self, key: &str, duration_ms: u64) -> Result<(), BrowserError> {
        self.session
            .browser()
            .keyboard()
            .hold_press(key, duration_ms, DelayRange::new(30, 80).unwrap())
            .await
            .map_err(|e| BrowserError::Action(e.to_string()))
    }

    pub async fn send_keys(&self, keys: &[String]) -> Result<(), BrowserError> {
        for key in keys {
            self.send_key(key, 50).await?;
        }
        Ok(())
    }

    pub async fn mouse_move(&self, x: f32, y: f32, steps: usize) -> Result<(), BrowserError> {
        // let ctx = self.active_context().into();
        self.session
            .browser()
            .mouse()
            .move_to(
                Point {
                    x: x as f64,
                    y: y as f64,
                },
                steps,
            )
            .await
            .map_err(|e| BrowserError::Action(e.to_string()))
    }

    pub async fn human_mouse_move(&self, x: f32, y: f32) -> Result<(), BrowserError> {
        // let ctx = self.active_context().into();
        let ctx = BrowsingContext::from_id(String::new(), CreateType::Tab);

        self.session
            .browser()
            .human_mouse()
            .move_to(
                Point {
                    x: x as f64,
                    y: y as f64,
                },
                ctx.id(),
                MouseMoveOptions::default(),
            )
            .await
            .map_err(|e| BrowserError::Action(e.to_string()))
    }

    pub async fn scroll_by(&self, y: i32, _human: bool) -> Result<(), BrowserError> {
        // let ctx = self.active_context().into();
        let ctx = BrowsingContext::from_id(String::new(), CreateType::Tab);
        self.session
            .browser()
            .human_mouse()
            .scroll(y, 0, ctx.id())
            .await
            .map_err(|e| BrowserError::Action(e.to_string()))
    }

    pub async fn scroll_to(&mut self, node_id: i64, human: bool) -> Result<(), BrowserError> {
        let mut node = self
            .session
            .browser_mut()
            .fetch_node(FetchNodeOptions::default().node_id(NodeId::new(node_id)))
            .await
            .map_err(|e| BrowserError::Action(e.to_string()))?;
        let position = node.get_position().await.ok_or_else(|| {
            BrowserError::Action(format!("Could not get position for: {node_id}"))
        })?;
        let target_y = position.y as i32;
        // I don't want to delve into the nuance of scroll right now !!! To Fix. !!!
        if false {
            let ctx = BrowsingContext::from_id(String::new(), CreateType::Tab);
            self.session
                .browser()
                .human_mouse()
                .scroll(target_y, 0, ctx.id())
                .await
                .map_err(|e| BrowserError::Action(e.to_string()))
        } else {
            node.scroll_into_view()
                .await
                .map_err(|e| BrowserError::Action(e.to_string()))
        }
    }

    pub async fn create_context(&mut self, url: &str) -> Result<String, BrowserError> {
        let browsing_ctx = self
            .session
            .browser_mut()
            .create_tab(url)
            .await
            .map_err(|e| BrowserError::Context(e.to_string()))?;
        let id: String = browsing_ctx.target_id().inner().to_owned();
        self.contexts.insert(id.clone(), browsing_ctx);
        Ok(id)
    }

    pub async fn close_context(&mut self, context_id: &str) -> Result<(), BrowserError> {
        let browsing_ctx = self
            .contexts
            .remove(context_id)
            .ok_or_else(|| BrowserError::Context(format!("Context not found: {context_id}")))?;
        // Incomplete feature
        // if let Err(e) = self
        //     .session
        //     .browser_mut()
        //     .c
        //     .close_context(browsing_ctx.clone())
        //     .await
        // {
        //     self.contexts.insert(context_id.to_string(), browsing_ctx);
        //     return Err(BrowserError::Context(e.to_string()));
        // }
        Ok(())
    }

    pub async fn find_node(&mut self, selector: &str) -> Result<i64, BrowserError> {
        // Locate by CSS selector, then extract the CDP NodeId for reuse in subsequent operations
        let node = self
            .session
            .browser_mut()
            .locate(Selector::Css(selector.to_string()))
            .await
            .map_err(|e| BrowserError::Action(e.to_string()))?
            .ok_or_else(|| BrowserError::Action(format!("Node not found: {selector}")))?;
        Ok(node.node_id().inner().to_owned())
    }

    pub async fn wait_for_node(
        &mut self,
        selector: &str,
        timeout_ms: u64,
    ) -> Result<i64, BrowserError> {
        // Wait until present, then extract CDP NodeId — same as find_node
        let node = self
            .session
            .browser_mut()
            .wait_for(
                Selector::Css(selector.to_string()),
                Duration::from_millis(timeout_ms),
            )
            .await
            .map_err(|e| BrowserError::Action(e.to_string()))?;
        Ok(node.node_id().inner().to_owned())
    }

    pub async fn fetch_html(&mut self, node_id: Option<i64>) -> Result<String, BrowserError> {
        let Some(id) = node_id else {
            // No node_id — return full document HTML
            let node = self
                .session
                .browser_mut()
                .locate(Selector::Css("html".to_string()))
                .await
                .map_err(|e| BrowserError::Action(e.to_string()))?
                .ok_or_else(|| BrowserError::Action("html element not found".into()))?;
            return Ok(node.get_html().await);
        };
        // Same fetch_node pattern as node_click / type_text
        let node = self
            .session
            .browser_mut()
            .fetch_node(FetchNodeOptions::default().node_id(NodeId::new(id)))
            .await
            .map_err(|e| BrowserError::Action(e.to_string()))?;
        Ok(node.get_html().await)
    }

    pub async fn fetch_text(&mut self, node_id: i64) -> Result<String, BrowserError> {
        // Same fetch_node pattern as node_click / type_text
        let node = self
            .session
            .browser_mut()
            .fetch_node(FetchNodeOptions::default().node_id(NodeId::new(node_id)))
            .await
            .map_err(|e| BrowserError::Action(e.to_string()))?;
        Ok(node.get_inner_text().await)
    }

    pub async fn get_ui_map(&mut self) -> Result<Vec<rusty_common::ui_map::UiNode>, BrowserError> {
        fn collect(nodes: &[AXNode], out: &mut Vec<UiNode>) {
            for n in nodes {
                let id = n.backend_dom_node_id.unwrap_or(0);
                let parent_id = n.parent_id.as_deref().and_then(|s| s.parse::<i64>().ok());
                let role = n
                    .role
                    .as_ref()
                    .and_then(|v| v.value.as_ref())
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let name = n
                    .name
                    .as_ref()
                    .and_then(|v| v.value.as_ref())
                    .and_then(|v| v.as_str())
                    .map(str::to_string);
                let value = n
                    .value
                    .as_ref()
                    .and_then(|v| v.value.as_ref())
                    .and_then(|v| v.as_str())
                    .map(str::to_string);
                let props: serde_json::Map<String, serde_json::Value> = n
                    .properties
                    .iter()
                    .filter_map(|p| {
                        Some((format!("{:?}", p.name), p.value.value.as_ref()?.clone()))
                    })
                    .collect();
                out.push(UiNode {
                    id,
                    role,
                    name,
                    parent_id,
                    value,
                    properties: if props.is_empty() { None } else { Some(props) },
                });
                collect(&n.children, out);
            }
        }

        let nodes = self
            .session
            .browser_mut()
            .get_accessible_nodes(true)
            .await
            .map_err(|e| BrowserError::Action(e.to_string()))?;
        let mut result = Vec::new();
        collect(&nodes, &mut result);
        Ok(result)
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
