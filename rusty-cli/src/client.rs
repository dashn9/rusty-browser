use anyhow::Result;
use reqwest::blocking::Client;
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

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base_url)
    }

    pub fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let resp = self.client.get(self.url(path))
            .header("X-API-Key", &self.api_key)
            .send()?;
        self.parse(resp)
    }

    pub fn post<T: DeserializeOwned>(&self, path: &str, body: &serde_json::Value) -> Result<T> {
        self.post_with_timeout(path, body, None)
    }

    pub fn post_with_timeout<T: DeserializeOwned>(&self, path: &str, body: &serde_json::Value, timeout: Option<std::time::Duration>) -> Result<T> {
        let mut req = self.client.post(self.url(path))
            .header("X-API-Key", &self.api_key)
            .json(body);
        if let Some(t) = timeout {
            req = req.timeout(t);
        }
        self.parse(req.send()?)
    }

    pub fn put<T: DeserializeOwned>(&self, path: &str, body: &serde_json::Value) -> Result<T> {
        let resp = self.client.put(self.url(path))
            .header("X-API-Key", &self.api_key)
            .json(body)
            .send()?;
        self.parse(resp)
    }

    pub fn delete<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let resp = self.client.delete(self.url(path))
            .header("X-API-Key", &self.api_key)
            .send()?;
        self.parse(resp)
    }

    fn parse<T: DeserializeOwned>(&self, resp: reqwest::blocking::Response) -> Result<T> {
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().unwrap_or_default();
            anyhow::bail!("HTTP {status}: {text}");
        }
        Ok(resp.json()?)
    }
}
