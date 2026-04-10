use std::pin::Pin;

use reqwest::Client;

use crate::{
    ai::{
        AIChatResponse, AIProvider, BrowserAction, ChatRequest, ContentPart, ImageUrl, Message,
        MessageContent,
    },
    config::AIConfig,
    error::AIError,
};

const SYSTEM_PROMPT: &str = r#"You are a browser automation agent. You analyze screenshots and determine what browser_actions to take to accomplish the user's instruction.

Respond with a JSON object containing:
- "reasoning": brief explanation of what you see and what you'll do
- "done": boolean, true if the instruction is complete
- "browser_actions": array of action objects

Action types:
- {"type": "navigate", "url": "..."}
- {"type": "click", "x": number, "y": number, "human": true/false}
- {"type": "type", "text": "...", "selector": "optional css selector"}
- {"type": "mouse_move", "x": number, "y": number}
- {"type": "scroll", "delta_x": number, "delta_y": number}
- {"type": "wait", "ms": number}
- {"type": "screenshot"}
- {"type": "done", "result": "description of outcome"}

Coordinates are in pixels relative to the viewport. Use human=true for natural mouse movements.
Only respond with valid JSON, no markdown or extra text."#;

#[derive(Clone)]
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
    fn build_messages(&self, screenshot_base64: &str, instruction: &str) -> Vec<Message> {
        let mut messages = vec![Message {
            role: "system".to_string(),
            content: MessageContent::Text(SYSTEM_PROMPT.to_string()),
        }];
        messages.push(Message {
            role: "user".to_string(),
            content: MessageContent::Parts(vec![
                ContentPart::Text {
                    text: format!("Instruction: {instruction}\n\nHere is the current screenshot of the browser:")
                },
                ContentPart::ImageUrl {
                    image_url: ImageUrl{
                        url: format!("data:image/jpeg;base64,{}", screenshot_base64)
                    }
                }
            ]
            )});
        messages
    }

    fn analyze_screenshot(
        &self,
        screenshot_base64: String,
        instruction: String,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<BrowserAction>, AIError>> + Send>> {

        // Talk about really bad design, eheh, you outdid yourself on this. 
        // Well to the caller, make sure you consume this future immediately before a serious obj mutation occurs
        let provider_clone = self.clone();
        Box::pin(async move {
            let messages =
                provider_clone.build_messages(screenshot_base64.as_ref(), instruction.as_ref());

            let url = format!("{}/chat/completions", provider_clone.base_url);

            let request = ChatRequest {
                model: provider_clone.model,
                messages,
            };

            let response = provider_clone
                .client
                .post(&url)
                .header("Authorization", format!("Bearer {}", provider_clone.api_key))
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
                return Err(AIError::RequestFailed(format!(
                    "status {}: {}",
                    status, body
                )));
            }

            let chat_response: AIChatResponse = response
                .json()
                .await
                .map_err(|e| AIError::InvalidResponse(e.to_string()))?;

            let raw_str = chat_response
                .choices
                .first()
                .map(|c| c.message.content.clone())
                .ok_or_else(|| AIError::InvalidResponse("no choices in response".to_string()))?;

            serde_json::from_str(&raw_str).map_err(|_| {
                AIError::InvalidResponse(
                    "Resonse did not fit Vector of Browser Actions".to_string(),
                )
            })
        })
    }
}
