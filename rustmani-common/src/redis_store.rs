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
        let conn = client.get_connection_manager().await?;
        Ok(Self { conn, prefix: prefix.to_string() })
    }

    fn key(&self, parts: &[&str]) -> String {
        format!("{}{}", self.prefix, parts.join(":"))
    }

    // --- Pending executions ---
    // Tracks spawned agents that haven't registered yet. No browser_id at this point —
    // the agent generates it and provides it on registration.

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

    /// Returns execution_ids spawned more than `older_than_secs` ago with no registration.
    pub async fn list_stale_agents(&self, older_than_secs: u64) -> Result<Vec<String>, StorageError> {
        let mut conn = self.conn.clone();
        let cutoff = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            .saturating_sub(older_than_secs);
        // ZRANGEBYSCORE pending_agents 0 <cutoff> — members whose score (spawn time) predates cutoff
        let ids: Vec<String> = redis::cmd("ZRANGEBYSCORE")
            .arg(self.key(&["pending_agents"]))
            .arg(0)
            .arg(cutoff)
            .query_async(&mut conn)
            .await?;
        Ok(ids)
    }

    // --- Browser ---
    // Created only once the agent registers with its real browser_id, host, and port.

    /// Insert or update a browser. Writes the execution_id → browser_id index as a permanent
    /// key and removes the execution from the stale-agent watch.
    pub async fn upsert_browser(&self, info: &BrowserInfo) -> Result<(), StorageError> {
        let mut conn = self.conn.clone();
        redis::pipe()
            // SADD browsers {browser_id} — track all known browser ids
            .sadd(self.key(&["browsers"]), &info.browser_id)
            // HMSET browser:{id} field val ... — store all fields in one round-trip
            .hset_multiple(self.key(&["browser", &info.browser_id]), &[
                ("execution_id", info.execution_id.as_str()),
                ("host", info.host.as_str()),
                ("grpc_port", &info.grpc_port.to_string()),
                ("state", info.state.as_str()),
            ])
            // SET execution:{id} browser_id — permanent reverse index (no TTL)
            .set(self.key(&["execution", &info.execution_id]), &info.browser_id)
            // ZREM pending_agents {execution_id} — agent registered, no longer a stale candidate
            .zrem(self.key(&["pending_agents"]), &info.execution_id)
            .exec_async(&mut conn)
            .await?;
        Ok(())
    }

    /// Accepts either a browser_id or an execution_id.
    pub async fn get_browser(&self, id: &str) -> Result<Option<BrowserInfo>, StorageError> {
        let mut conn = self.conn.clone();

        // HMGET browser:{id} field ... — fetch multiple hash fields in one round-trip; nil for missing
        let fields: Vec<Option<String>> = redis::cmd("HMGET")
            .arg(self.key(&["browser", id]))
            .arg(&["execution_id", "host", "grpc_port", "state"])
            .query_async(&mut conn)
            .await?;

        if let Some(info) = self.hydrate(id, fields).await? {
            return Ok(Some(info));
        }

        // execution_id field was nil — id might be an execution_id, look up the browser_id
        let Some(bid): Option<String> = conn.get(self.key(&["execution", id])).await? else {
            return Ok(None);
        };

        let fields: Vec<Option<String>> = redis::cmd("HMGET")
            .arg(self.key(&["browser", &bid]))
            .arg(&["execution_id", "host", "grpc_port", "state"])
            .query_async(&mut conn)
            .await?;

        self.hydrate(&bid, fields).await
    }

    /// Builds a `BrowserInfo` from raw HMGET fields. Returns `None` if any required
    /// connection field (execution_id, host, grpc_port) is absent — a partial record
    /// is not a valid browser.
    async fn hydrate(&self, browser_id: &str, fields: Vec<Option<String>>) -> Result<Option<BrowserInfo>, StorageError> {
        let Some(execution_id) = fields[0].clone() else { return Ok(None); };
        let Some(host) = fields[1].clone() else { return Ok(None); };
        let Some(grpc_port) = fields[2].as_deref().and_then(|p| p.parse().ok()) else { return Ok(None); };
        let state = BrowserState::from_str(fields[3].as_deref().unwrap_or("idle"));
        let contexts = self.list_contexts(browser_id).await.unwrap_or_default();
        Ok(Some(BrowserInfo { browser_id: browser_id.to_string(), execution_id, host, grpc_port, state, contexts }))
    }

    pub async fn list_browsers(&self) -> Result<Vec<BrowserInfo>, StorageError> {
        let mut conn = self.conn.clone();
        // SMEMBERS browsers — all registered browser ids
        let ids: Vec<String> = conn.smembers(self.key(&["browsers"])).await?;
        let mut browsers = Vec::with_capacity(ids.len());
        for id in ids {
            if let Ok(Some(b)) = self.get_browser(&id).await {
                browsers.push(b);
            }
        }
        Ok(browsers)
    }

    pub async fn update_browser_state(&self, browser_id: &str, state: &BrowserState) -> Result<(), StorageError> {
        let mut conn = self.conn.clone();
        let _: () = conn.hset(self.key(&["browser", browser_id]), "state", state.as_str()).await?;
        Ok(())
    }

    pub async fn remove_browser(&self, browser_id: &str) -> Result<(), StorageError> {
        let mut conn = self.conn.clone();
        let execution_id: Option<String> = conn
            .hget(self.key(&["browser", browser_id]), "execution_id")
            .await?;
        let mut pipe = redis::pipe();
        pipe.srem(self.key(&["browsers"]), browser_id)
            .del(self.key(&["browser", browser_id]))
            .del(self.key(&["browser", browser_id, "contexts"]));
        if let Some(eid) = &execution_id {
            // DEL execution:{id} — clean up the reverse index
            pipe.del(self.key(&["execution", eid]));
        }
        pipe.exec_async(&mut conn).await?;
        Ok(())
    }

    // --- Contexts ---

    pub async fn add_context(&self, browser_id: &str, context_id: &str) -> Result<(), StorageError> {
        let mut conn = self.conn.clone();
        let _: () = conn.sadd(self.key(&["browser", browser_id, "contexts"]), context_id).await?;
        Ok(())
    }

    pub async fn remove_context(&self, browser_id: &str, context_id: &str) -> Result<(), StorageError> {
        let mut conn = self.conn.clone();
        let _: () = conn.srem(self.key(&["browser", browser_id, "contexts"]), context_id).await?;
        Ok(())
    }

    pub async fn list_contexts(&self, browser_id: &str) -> Result<Vec<String>, StorageError> {
        let mut conn = self.conn.clone();
        Ok(conn.smembers(self.key(&["browser", browser_id, "contexts"])).await?)
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
        browser_id: &str,
        status: &str,
        step: u32,
        max_steps: u32,
        instruction: &str,
        reasoning: &str,
    ) -> Result<(), StorageError> {
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
    ) -> Result<Option<std::collections::HashMap<String, String>>, StorageError> {
        let mut conn = self.conn.clone();
        let key = self.key(&["instruct", browser_id]);
        let exists: bool = conn.exists(&key).await?;
        if !exists { return Ok(None); }
        Ok(Some(conn.hgetall(&key).await?))
    }

    pub async fn push_instruct_history(&self, browser_id: &str, message_json: &str) -> Result<(), StorageError> {
        let mut conn = self.conn.clone();
        // RPUSH — append to the right end of the list, preserving chronological order
        let _: () = conn.rpush(self.key(&["instruct", browser_id, "history"]), message_json).await?;
        Ok(())
    }

    pub async fn clear_instruct(&self, browser_id: &str) -> Result<(), StorageError> {
        let mut conn = self.conn.clone();
        redis::pipe()
            .del(self.key(&["instruct", browser_id]))
            .del(self.key(&["instruct", browser_id, "history"]))
            .exec_async(&mut conn)
            .await?;
        Ok(())
    }
}
