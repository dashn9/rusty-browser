use anyhow::Result;
use reqwest::Client;
use serde::de::DeserializeOwned;

pub struct RustyClient {
    client: Client,
    base_url: String,
    api_key: String,
}

impl RustyClient {
    pub fn new(base_url: &str, api_key: &str) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key: api_key.to_string(),
        }
    }

    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let resp = self
            .client
            .get(format!("{}{path}", self.base_url))
            .header("X-API-Key", &self.api_key)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("HTTP {status}: {text}");
        }

        Ok(resp.json().await?)
    }

    pub async fn post<T: DeserializeOwned>(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<T> {
        let resp = self
            .client
            .post(format!("{}{path}", self.base_url))
            .header("X-API-Key", &self.api_key)
            .json(body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("HTTP {status}: {text}");
        }

        Ok(resp.json().await?)
    }

    pub async fn delete<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let resp = self
            .client
            .delete(format!("{}{path}", self.base_url))
            .header("X-API-Key", &self.api_key)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("HTTP {status}: {text}");
        }

        Ok(resp.json().await?)
    }
}
