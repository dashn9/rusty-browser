pub mod openai;
pub mod openrouter;

use std::pin::Pin;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{config::{AIConfig, AIProviderKind}, error::AIError};

// ---------- Messages ----------

#[derive(Serialize, Deserialize, Clone)]
pub struct Message {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl Message {
    pub fn system(text: &str) -> Self {
        Self {
            role: "system".to_string(),
            content: Some(json!(text)),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    pub fn user_with_screenshot(instruction: &str, screenshot_b64: &str) -> Self {
        Self {
            role: "user".to_string(),
            content: Some(json!([
                { "type": "text", "text": format!("Instruction: {instruction}") },
                { "type": "image_url", "image_url": { "url": format!("data:image/jpeg;base64,{screenshot_b64}") } }
            ])),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    pub fn screenshot_update(screenshot_b64: &str) -> Self {
        Self {
            role: "user".to_string(),
            content: Some(json!([
                { "type": "text", "text": "Current browser state after your last actions:" },
                { "type": "image_url", "image_url": { "url": format!("data:image/jpeg;base64,{screenshot_b64}") } }
            ])),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    pub fn assistant_tool_calls(calls: Vec<ToolCall>) -> Self {
        Self {
            role: "assistant".to_string(),
            content: None,
            tool_calls: Some(calls),
            tool_call_id: None,
        }
    }

    pub fn tool_result(tool_call_id: impl Into<String>, result: impl Into<String>) -> Self {
        Self {
            role: "tool".to_string(),
            content: Some(json!(result.into())),
            tool_calls: None,
            tool_call_id: Some(tool_call_id.into()),
        }
    }
}

// ---------- Tool calls ----------

#[derive(Serialize, Deserialize, Clone)]
pub struct ToolCall {
    pub id: String,
    pub r#type: String,
    pub function: ToolCallFunction,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ToolCallFunction {
    pub name: String,
    pub arguments: String,
}

// ---------- Tool definitions ----------

#[derive(Serialize)]
pub(crate) struct Tool {
    pub r#type: &'static str,
    pub function: ToolDef,
}

#[derive(Serialize)]
pub(crate) struct ToolDef {
    pub name: &'static str,
    pub description: &'static str,
    pub parameters: Value,
}

pub fn browser_tools() -> Vec<Tool> {
    vec![
        Tool {
            r#type: "function",
            function: ToolDef {
                name: "navigate",
                description: "Navigate the browser to a URL",
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "url": { "type": "string" }
                    },
                    "required": ["url"]
                }),
            },
        },
        Tool {
            r#type: "function",
            function: ToolDef {
                name: "click",
                description: "Click at pixel coordinates on the screen",
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "x": { "type": "number" },
                        "y": { "type": "number" }
                    },
                    "required": ["x", "y"]
                }),
            },
        },
        Tool {
            r#type: "function",
            function: ToolDef {
                name: "node_click",
                description: "Click a DOM element by CSS selector",
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "selector": { "type": "string" }
                    },
                    "required": ["selector"]
                }),
            },
        },
        Tool {
            r#type: "function",
            function: ToolDef {
                name: "type",
                description: "Type text, optionally into a specific input element",
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "text": { "type": "string" },
                        "selector": { "type": "string", "description": "CSS selector of the input (omit to type at current focus)" }
                    },
                    "required": ["text"]
                }),
            },
        },
        Tool {
            r#type: "function",
            function: ToolDef {
                name: "scroll_by",
                description: "Scroll the page vertically. Positive = down, negative = up.",
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "y": { "type": "integer" }
                    },
                    "required": ["y"]
                }),
            },
        },
        Tool {
            r#type: "function",
            function: ToolDef {
                name: "scroll_to",
                description: "Scroll a DOM element into the viewport",
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "selector": { "type": "string" },
                        "to": { "type": "integer", "description": "0 = align top, 100 = align bottom" }
                    },
                    "required": ["selector", "to"]
                }),
            },
        },
        Tool {
            r#type: "function",
            function: ToolDef {
                name: "fetch_html",
                description: "Return the inner HTML of a DOM element",
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "selector": { "type": "string", "description": "CSS selector (omit for full document body)" }
                    }
                }),
            },
        },
        Tool {
            r#type: "function",
            function: ToolDef {
                name: "fetch_text",
                description: "Return the visible text content of a DOM element",
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "selector": { "type": "string" }
                    },
                    "required": ["selector"]
                }),
            },
        },
        Tool {
            r#type: "function",
            function: ToolDef {
                name: "eval_js",
                description: "Execute JavaScript in the browser and return the result",
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "script": { "type": "string" }
                    },
                    "required": ["script"]
                }),
            },
        },
        Tool {
            r#type: "function",
            function: ToolDef {
                name: "wait_for_node",
                description: "Wait until a CSS selector appears in the DOM",
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "selector": { "type": "string" },
                        "timeout_ms": { "type": "integer" }
                    },
                    "required": ["selector", "timeout_ms"]
                }),
            },
        },
        Tool {
            r#type: "function",
            function: ToolDef {
                name: "wait",
                description: "Pause for a number of milliseconds",
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "ms": { "type": "integer" }
                    },
                    "required": ["ms"]
                }),
            },
        },
        Tool {
            r#type: "function",
            function: ToolDef {
                name: "screenshot",
                description: "Capture a fresh screenshot to see the current browser state",
                parameters: json!({ "type": "object", "properties": {} }),
            },
        },
        Tool {
            r#type: "function",
            function: ToolDef {
                name: "done",
                description: "Signal that the instruction is complete",
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "result": { "type": "string", "description": "What was accomplished" }
                    },
                    "required": ["result"]
                }),
            },
        },
    ]
}

// ---------- Actions ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BrowserAction {
    Navigate { url: String },
    Click { x: f32, y: f32, human: bool },
    NodeClick { selector: String, human: bool },
    Type { text: String, selector: Option<String> },
    MouseMove { x: f32, y: f32 },
    HumanMouseMove { x: f32, y: f32 },
    ScrollBy { y: i32, human: bool },
    ScrollTo { selector: String, human: bool, to: u32 },
    FetchHtml { selector: Option<String> },
    FetchText { selector: String },
    EvalJs { script: String },
    FindNode { selector: String },
    WaitForNode { selector: String, timeout_ms: u64 },
    Wait { ms: u64 },
    Screenshot,
    Done { result: String },
}

/// Convert a single tool call into a BrowserAction, injecting server-side defaults.
pub fn parse_action(call: &ToolCall) -> Result<BrowserAction, AIError> {
    let mut v: Value = serde_json::from_str(&call.function.arguments).map_err(|e| {
        AIError::InvalidResponse(format!("bad args for '{}': {e}", call.function.name))
    })?;

    // AI doesn't control human-like movement — always on for input actions.
    match call.function.name.as_str() {
        "click" | "node_click" | "scroll_by" | "scroll_to" => {
            v["human"] = json!(true);
        }
        _ => {}
    }

    v["type"] = Value::String(call.function.name.clone());
    serde_json::from_value(v).map_err(|e| {
        AIError::InvalidResponse(format!("unknown tool '{}': {e}", call.function.name))
    })
}

// ---------- Provider trait ----------

#[derive(Serialize)]
pub(crate) struct ChatRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub tools: Vec<Tool>,
    pub tool_choice: &'static str,
}

#[derive(Deserialize)]
pub(crate) struct AIChatResponse {
    pub choices: Vec<Choice>,
}

#[derive(Deserialize)]
pub(crate) struct Choice {
    pub message: ResponseMessage,
}

#[derive(Deserialize)]
pub(crate) struct ResponseMessage {
    #[serde(default)]
    pub tool_calls: Vec<ToolCall>,
}

pub trait AIProvider: Send + Sync {
    /// Send the full conversation and receive the model's next tool calls.
    fn chat(
        &self,
        messages: Vec<Message>,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<ToolCall>, AIError>> + Send>>;
}

pub fn create_provider(config: &AIConfig) -> Box<dyn AIProvider> {
    match config.provider {
        AIProviderKind::OpenAI => Box::new(openai::OpenAIProvider::new(config)),
        AIProviderKind::OpenRouter => Box::new(openrouter::OpenRouterProvider::new(config)),
    }
}
