use std::pin::Pin;

use reqwest::Client;

use crate::{
    ai::{AIChatResponse, AIProvider, ChatRequest, Message, ToolCall, browser_tools},
    config::AIConfig,
    error::AIError,
};

pub struct OpenRouterProvider {
    client: Client,
    api_key: String,
    model: String,
    base_url: String,
}

impl OpenRouterProvider {
    pub fn new(config: &AIConfig) -> Self {
        Self {
            client: Client::new(),
            api_key: config.api_key.clone(),
            model: config.model.clone(),
            base_url: config
                .base_url
                .clone()
                .unwrap_or_else(|| "https://openrouter.ai/api/v1".to_string()),
        }
    }
}

impl AIProvider for OpenRouterProvider {
    fn chat(
        &self,
        messages: Vec<Message>,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<ToolCall>, AIError>> + Send>> {
        let client = self.client.clone();
        let api_key = self.api_key.clone();
        let url = format!("{}/chat/completions", self.base_url);
        let model = self.model.clone();

        Box::pin(async move {
            let request = ChatRequest {
                model,
                messages,
                tools: browser_tools(),
                tool_choice: "required",
            };

            let response = client
                .post(&url)
                .header("Authorization", format!("Bearer {api_key}"))
                .json(&request)
                .send()
                .await
                .map_err(|e| AIError::RequestFailed(e.to_string()))?;

            let status = response.status();
            if status == reqwest::StatusCode::UNAUTHORIZED {
                return Err(AIError::Unauthorized);
            }
            if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                return Err(AIError::RateLimited);
            }
            if !status.is_success() {
                let body = response.text().await.unwrap_or_default();
                return Err(AIError::RequestFailed(format!("status {status}: {body}")));
            }

            let chat_response: AIChatResponse = response
                .json()
                .await
                .map_err(|e| AIError::InvalidResponse(e.to_string()))?;

            let tool_calls = chat_response
                .choices
                .into_iter()
                .next()
                .map(|c| c.message.tool_calls)
                .ok_or_else(|| AIError::InvalidResponse("no choices in response".to_string()))?;

            if tool_calls.is_empty() {
                return Err(AIError::InvalidResponse(
                    "model returned no tool calls".to_string(),
                ));
            }

            Ok(tool_calls)
        })
    }
}
