pub mod openai;
pub mod openrouter;

use std::pin::Pin;

use serde::{Deserialize, Serialize};

use crate::{config::{AIConfig, AIProviderKind}, error::AIError};

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<Message>,
}

#[derive(Serialize, Clone)]
struct Message {
    role: String,
    content: MessageContent,
}

#[derive(Serialize, Clone)]
#[serde(untagged)]
enum MessageContent {
    Text(String),
    Parts(Vec<ContentPart>),
}

#[derive(Serialize, Clone)]
#[serde(tag = "type")]
enum ContentPart {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image_url")]
    ImageUrl { image_url: ImageUrl },
}

#[derive(Serialize, Clone)]
struct ImageUrl {
    url: String,
}

#[derive(Deserialize)]
struct AIChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: ResponseMessage,
}

#[derive(Deserialize)]
struct ResponseMessage {
    content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BrowserAction {
    Navigate { url: String },
    Click { x: f32, y: f32, human: bool },
    Type { text: String, selector: Option<String> },
    MouseMove { x: f32, y: f32 },
    Scroll { delta_x: f32, delta_y: f32 },
    Wait { ms: u64 },
    Screenshot,
    Done { result: String },
}

pub trait AIProvider: Send + Sync {
    fn build_messages(&self, screenshot_base64: &str, instruction: &str) -> Vec<Message>;
    fn analyze_screenshot(
        &self,
        screenshot_base64: String,
        instruction: String,
    ) -> Box<dyn Future<Output = Result<Vec<BrowserAction>, AIError>>>;
}

pub fn create_provider(config: &AIConfig) -> Box<dyn AIProvider> {
    match config.provider {
        AIProviderKind::OpenAI => Box::new(openai::OpenAIProvider::new(config)),
        AIProviderKind::OpenRouter => Box::new(openrouter::OpenRouterProvider::new(config)),
    }
}
