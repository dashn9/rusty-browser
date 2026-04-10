use redis::aio::ConnectionManager;
use redis::AsyncCommands;

use crate::error::StorageError;
use crate::state::{BrowserInfo, BrowserState};

#[derive(Clone)]
pub struct RedisStore {
    conn: ConnectionManager,
    prefix: String,
}

impl RedisStore {
    pub async fn new(url: &str, prefix: &str) -> Result<Self, StorageError> {
        let client = redis::Client::open(url)?;
        let config = redis::aio::ConnectionManagerConfig::new()
            .set_connection_timeout(Some(std::time::Duration::from_secs(5)))
            .set_response_timeout(Some(std::time::Duration::from_secs(5)))
            .set_number_of_retries(3);
        let conn = client.get_connection_manager_with_config(config).await?;
        Ok(Self { conn, prefix: prefix.to_string() })
    }

    fn key(&self, parts: &[&str]) -> String {
        format!("{}{}", self.prefix, parts.join(":"))
    }

    // --- Pending executions ---
    // Tracks spawned agents that haven't registered yet.

    pub async fn store_pending_execution(&self, execution_id: &str) -> Result<(), StorageError> {
        let mut conn = self.conn.clone();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        // ZADD pending_agents <unix_ts> <execution_id> — scored set lets us range-query by spawn time
        let _: () = conn.zadd(self.key(&["pending_agents"]), execution_id, now).await?;
        Ok(())
    }

    pub async fn list_pending_agents(&self) -> Result<Vec<String>, StorageError> {
        let mut conn = self.conn.clone();
        let ids: Vec<String> = redis::cmd("ZRANGEBYSCORE")
            .arg(self.key(&["pending_agents"]))
            .arg("-inf")
            .arg("+inf")
            .query_async(&mut conn)
            .await?;
        Ok(ids)
    }

    pub async fn clear_pending_agents(&self) -> Result<(), StorageError> {
        let mut conn = self.conn.clone();
        let _: () = conn.del(self.key(&["pending_agents"])).await?;
        Ok(())
    }

    /// Returns execution_ids spawned more than `older_than_secs` ago with no registration.
    pub async fn list_stale_agents(&self, older_than_secs: u64) -> Result<Vec<String>, StorageError> {
        let mut conn = self.conn.clone();
        let cutoff = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            .saturating_sub(older_than_secs);
        // ZRANGEBYSCORE pending_agents 0 <cutoff> — members whose score predates the cutoff
        let ids: Vec<String> = redis::cmd("ZRANGEBYSCORE")
            .arg(self.key(&["pending_agents"]))
            .arg(0)
            .arg(cutoff)
            .query_async(&mut conn)
            .await?;
        Ok(ids)
    }

    // --- Browser ---
    // execution_id is the primary key. browser_id is stored as metadata only.

    /// Insert or update a browser record keyed by execution_id.
    pub async fn upsert_browser(&self, info: &BrowserInfo) -> Result<(), StorageError> {
        let mut conn = self.conn.clone();
        redis::pipe()
            // SADD browsers {execution_id} — track all registered executions
            .sadd(self.key(&["browsers"]), &info.execution_id)
            // HMSET browser:{execution_id} — all fields in one round-trip
            .hset_multiple(self.key(&["browser", &info.execution_id]), &[
                ("browser_id", info.browser_id.as_str()),
                ("public_ip", info.public_ip.as_str()),
                ("private_ip", info.private_ip.as_str()),
                ("grpc_port", &info.grpc_port.to_string()),
                ("state", info.state.as_str()),
            ])
            // ZREM pending_agents {execution_id} — agent registered, no longer stale
            .zrem(self.key(&["pending_agents"]), &info.execution_id)
            .exec_async(&mut conn)
            .await?;
        Ok(())
    }

    pub async fn get_browser(&self, execution_id: &str) -> Result<Option<BrowserInfo>, StorageError> {
        let mut conn = self.conn.clone();
        // HMGET browser:{execution_id} field ... — nil for missing fields
        let fields: Vec<Option<String>> = redis::cmd("HMGET")
            .arg(self.key(&["browser", execution_id]))
            .arg(&["browser_id", "public_ip", "private_ip", "grpc_port", "state"])
            .query_async(&mut conn)
            .await?;
        self.hydrate(execution_id, fields).await
    }

    /// Builds a `BrowserInfo` from raw HMGET fields. Returns `None` if any required
    /// connection field is absent — a partial record is not a usable browser.
    async fn hydrate(&self, execution_id: &str, fields: Vec<Option<String>>) -> Result<Option<BrowserInfo>, StorageError> {
        let Some(browser_id) = fields[0].clone() else { return Ok(None); };
        let Some(public_ip) = fields[1].clone() else { return Ok(None); };
        let Some(private_ip) = fields[2].clone() else { return Ok(None); };
        let Some(grpc_port) = fields[3].as_deref().and_then(|p| p.parse().ok()) else { return Ok(None); };
        let state = BrowserState::from_str(fields[4].as_deref().unwrap_or("idle"));
        let contexts = self.list_contexts(execution_id).await.unwrap_or_default();
        Ok(Some(BrowserInfo { execution_id: execution_id.to_string(), browser_id, public_ip, private_ip, grpc_port, state, contexts }))
    }

    pub async fn list_browsers(&self) -> Result<Vec<BrowserInfo>, StorageError> {
        let mut conn = self.conn.clone();
        // SMEMBERS browsers — all registered execution ids
        let ids: Vec<String> = conn.smembers(self.key(&["browsers"])).await?;
        let mut browsers = Vec::with_capacity(ids.len());
        for id in ids {
            if let Ok(Some(b)) = self.get_browser(&id).await {
                browsers.push(b);
            }
        }
        Ok(browsers)
    }

    pub async fn update_browser_state(&self, execution_id: &str, state: &BrowserState) -> Result<(), StorageError> {
        let mut conn = self.conn.clone();
        let _: () = conn.hset(self.key(&["browser", execution_id]), "state", state.as_str()).await?;
        Ok(())
    }

    pub async fn remove_browser(&self, execution_id: &str) -> Result<(), StorageError> {
        let mut conn = self.conn.clone();
        // SREM/DEL/ZREM all return 0 (not an error) when the key or member is absent,
        // so this is safe to call even if the browser was never registered.
        redis::pipe()
            .srem(self.key(&["browsers"]), execution_id)
            .del(self.key(&["browser", execution_id]))
            .del(self.key(&["browser", execution_id, "contexts"]))
            .zrem(self.key(&["pending_agents"]), execution_id)
            .exec_async(&mut conn)
            .await?;
        Ok(())
    }

    // --- Contexts ---

    pub async fn add_context(&self, execution_id: &str, context_id: &str) -> Result<(), StorageError> {
        let mut conn = self.conn.clone();
        let _: () = conn.sadd(self.key(&["browser", execution_id, "contexts"]), context_id).await?;
        Ok(())
    }

    pub async fn remove_context(&self, execution_id: &str, context_id: &str) -> Result<(), StorageError> {
        let mut conn = self.conn.clone();
        let _: () = conn.srem(self.key(&["browser", execution_id, "contexts"]), context_id).await?;
        Ok(())
    }

    pub async fn list_contexts(&self, execution_id: &str) -> Result<Vec<String>, StorageError> {
        let mut conn = self.conn.clone();
        Ok(conn.smembers(self.key(&["browser", execution_id, "contexts"])).await?)
    }

    // --- TLS cert (cluster-wide) ---

    // agent cert (server → agent TLS)
    pub async fn set_tls_cert(&self, cert_pem: &str) -> Result<(), StorageError> {
        let mut conn = self.conn.clone();
        let _: () = conn.set(self.key(&["tls_cert"]), cert_pem).await?;
        Ok(())
    }

    pub async fn get_tls_cert(&self) -> Result<Option<String>, StorageError> {
        let mut conn = self.conn.clone();
        Ok(conn.get(self.key(&["tls_cert"])).await?)
    }

    // master cert (agent → master TLS)
    pub async fn set_master_tls_cert(&self, cert_pem: &str, key_pem: &str) -> Result<(), StorageError> {
        let mut conn = self.conn.clone();
        redis::pipe()
            .set(self.key(&["master_tls_cert"]), cert_pem)
            .set(self.key(&["master_tls_key"]), key_pem)
            .exec_async(&mut conn)
            .await?;
        Ok(())
    }

    pub async fn get_master_tls_cert(&self) -> Result<Option<(String, String)>, StorageError> {
        let mut conn = self.conn.clone();
        let cert: Option<String> = conn.get(self.key(&["master_tls_cert"])).await?;
        let key: Option<String> = conn.get(self.key(&["master_tls_key"])).await?;
        Ok(match (cert, key) {
            (Some(c), Some(k)) => Some((c, k)),
            _ => None,
        })
    }

    // --- Instruct state ---

    pub async fn set_instruct_state(
        &self,
        execution_id: &str,
        status: &str,
        step: u32,
        max_steps: u32,
        instruction: &str,
        reasoning: &str,
    ) -> Result<(), StorageError> {
        let mut conn = self.conn.clone();
        let _: () = conn.hset_multiple(self.key(&["instruct", execution_id]), &[
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
        execution_id: &str,
    ) -> Result<Option<std::collections::HashMap<String, String>>, StorageError> {
        let mut conn = self.conn.clone();
        let key = self.key(&["instruct", execution_id]);
        let exists: bool = conn.exists(&key).await?;
        if !exists { return Ok(None); }
        Ok(Some(conn.hgetall(&key).await?))
    }

    pub async fn push_instruct_history(&self, execution_id: &str, message_json: &str) -> Result<(), StorageError> {
        let mut conn = self.conn.clone();
        // RPUSH — append to the right end of the list, preserving chronological order
        let _: () = conn.rpush(self.key(&["instruct", execution_id, "history"]), message_json).await?;
        Ok(())
    }

    pub async fn clear_instruct(&self, execution_id: &str) -> Result<(), StorageError> {
        let mut conn = self.conn.clone();
        redis::pipe()
            .del(self.key(&["instruct", execution_id]))
            .del(self.key(&["instruct", execution_id, "history"]))
            .exec_async(&mut conn)
            .await?;
        Ok(())
    }
}
