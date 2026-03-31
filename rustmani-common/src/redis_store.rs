use redis::aio::ConnectionManager;
use redis::AsyncCommands;

use crate::error::RustmaniError;
use crate::state::*;

#[derive(Clone)]
pub struct RedisStore {
    conn: ConnectionManager,
    prefix: String,
}

impl RedisStore {
    pub async fn new(url: &str, prefix: &str) -> Result<Self, RustmaniError> {
        let client = redis::Client::open(url)?;
        let conn = client.get_connection_manager().await?;
        Ok(Self { conn, prefix: prefix.to_string() })
    }

    fn key(&self, parts: &[&str]) -> String {
        format!("{}{}", self.prefix, parts.join(":"))
    }

    // --- Browser ---

    pub async fn add_browser(&self, info: &BrowserInfo) -> Result<(), RustmaniError> {
        let mut conn = self.conn.clone();
        redis::pipe()
            .sadd(self.key(&["browsers"]), &info.browser_id)
            .hset_multiple(self.key(&["browser", &info.browser_id]), &[
                ("host", info.host.as_str()),
                ("grpc_port", &info.grpc_port.to_string()),
                ("state", info.state.as_str()),
            ])
            .exec_async(&mut conn)
            .await?;
        Ok(())
    }

    pub async fn get_browser(&self, browser_id: &str) -> Result<BrowserInfo, RustmaniError> {
        let mut conn = self.conn.clone();
        let fields: Vec<Option<String>> = redis::cmd("HMGET")
            .arg(self.key(&["browser", browser_id]))
            .arg("host").arg("grpc_port").arg("state")
            .query_async(&mut conn)
            .await?;

        let host = fields[0].clone()
            .ok_or_else(|| RustmaniError::BrowserNotFound(browser_id.to_string()))?;
        let grpc_port = fields[1].as_deref().unwrap_or("50051").parse().unwrap_or(50051);
        let state = BrowserState::from_str(fields[2].as_deref().unwrap_or("idle"));
        let contexts = self.list_contexts(browser_id).await.unwrap_or_default();

        Ok(BrowserInfo { browser_id: browser_id.to_string(), host, grpc_port, state, contexts })
    }

    pub async fn list_browsers(&self) -> Result<Vec<BrowserInfo>, RustmaniError> {
        let mut conn = self.conn.clone();
        let ids: Vec<String> = conn.smembers(self.key(&["browsers"])).await?;
        let mut browsers = Vec::with_capacity(ids.len());
        for id in ids {
            match self.get_browser(&id).await {
                Ok(b) => browsers.push(b),
                Err(RustmaniError::BrowserNotFound(_)) => continue,
                Err(e) => return Err(e),
            }
        }
        Ok(browsers)
    }

    pub async fn update_browser_state(
        &self,
        browser_id: &str,
        state: &BrowserState,
    ) -> Result<(), RustmaniError> {
        let mut conn = self.conn.clone();
        let _: () = conn
            .hset(self.key(&["browser", browser_id]), "state", state.as_str())
            .await?;
        Ok(())
    }

    pub async fn remove_browser(&self, browser_id: &str) -> Result<(), RustmaniError> {
        let mut conn = self.conn.clone();
        redis::pipe()
            .srem(self.key(&["browsers"]), browser_id)
            .del(self.key(&["browser", browser_id]))
            .del(self.key(&["browser", browser_id, "contexts"]))
            .exec_async(&mut conn)
            .await?;
        Ok(())
    }

    // --- Contexts (stored under browser_id) ---

    pub async fn add_context(&self, browser_id: &str, context_id: &str) -> Result<(), RustmaniError> {
        let mut conn = self.conn.clone();
        let _: () = conn
            .sadd(self.key(&["browser", browser_id, "contexts"]), context_id)
            .await?;
        Ok(())
    }

    pub async fn remove_context(
        &self,
        browser_id: &str,
        context_id: &str,
    ) -> Result<(), RustmaniError> {
        let mut conn = self.conn.clone();
        let _: () = conn
            .srem(self.key(&["browser", browser_id, "contexts"]), context_id)
            .await?;
        Ok(())
    }

    pub async fn list_contexts(&self, browser_id: &str) -> Result<Vec<String>, RustmaniError> {
        let mut conn = self.conn.clone();
        let ids = conn.smembers(self.key(&["browser", browser_id, "contexts"])).await?;
        Ok(ids)
    }

    // --- Instruct state (keyed by browser_id) ---

    pub async fn set_instruct_state(
        &self,
        browser_id: &str,
        status: &str,
        step: u32,
        max_steps: u32,
        instruction: &str,
        reasoning: &str,
    ) -> Result<(), RustmaniError> {
        let mut conn = self.conn.clone();
        let _: () = conn.hset_multiple(self.key(&["instruct", browser_id]), &[
            ("status", status),
            ("current_step", &step.to_string()),
            ("max_steps", &max_steps.to_string()),
            ("instruction", instruction),
            ("last_reasoning", reasoning),
        ]).await?;
        Ok(())
    }

    pub async fn get_instruct_state(
        &self,
        browser_id: &str,
    ) -> Result<Option<std::collections::HashMap<String, String>>, RustmaniError> {
        let mut conn = self.conn.clone();
        let key = self.key(&["instruct", browser_id]);
        let exists: bool = conn.exists(&key).await?;
        if !exists { return Ok(None); }
        Ok(Some(conn.hgetall(&key).await?))
    }

    pub async fn push_instruct_history(
        &self,
        browser_id: &str,
        message_json: &str,
    ) -> Result<(), RustmaniError> {
        let mut conn = self.conn.clone();
        let _: () = conn
            .rpush(self.key(&["instruct", browser_id, "history"]), message_json)
            .await?;
        Ok(())
    }

    pub async fn clear_instruct(&self, browser_id: &str) -> Result<(), RustmaniError> {
        let mut conn = self.conn.clone();
        redis::pipe()
            .del(self.key(&["instruct", browser_id]))
            .del(self.key(&["instruct", browser_id, "history"]))
            .exec_async(&mut conn)
            .await?;
        Ok(())
    }
}
