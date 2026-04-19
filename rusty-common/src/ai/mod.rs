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

    pub fn user(text: &str) -> Self {
        Self {
            role: "user".to_string(),
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ToolCall {
    pub id: String,
    pub r#type: String,
    pub function: ToolCallFunction,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
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
                description: "Click a DOM element by node_id returned from find_node or wait_for_node",
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "node_id": { "type": "integer" }
                    },
                    "required": ["node_id"]
                }),
            },
        },
        Tool {
            r#type: "function",
            function: ToolDef {
                name: "type",
                description: "Types text into whichever element currently has focus.",
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "text": { "type": "string" },
                        // "node_id": { "type": "integer", "description": "node_id from find_node (omit to type at current focus)" }
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
                description: "Scroll a DOM element into the viewport by node_id",
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "node_id": { "type": "integer" }
                    },
                    "required": ["node_id"]
                }),
            },
        },
        // Tool {
        //     r#type: "function",
        //     function: ToolDef {
        //         name: "send_keys",
        //         description: r#"Send one or more discrete key presses.
        //
        // Acceptable keys:
        // - Backspace
        //
        // Formats:
        // - Single: "Backspace"
        // - Comma-separated: "Backspace, Backspace, Backspace"
        // - With count: "Backspace10" (sends Backspace 10 times)"#,
        //         parameters: json!({
        //             "type": "object",
        //             "properties": {
        //                 "keys": { "type": "string" }
        //             },
        //             "required": ["keys"]
        //         }),
        //     },
        // },
        Tool {
            r#type: "function",
            function: ToolDef {
                name: "hold_key",
                description: r#"Hold a key down for a duration. Use this to clear an input field.

Acceptable keys:
- Backspace

Format: "Backspace3000" (holds Backspace for 3000ms)"#,
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "key": { "type": "string" }
                    },
                    "required": ["key"]
                }),
            },
        },
        Tool {
            r#type: "function",
            function: ToolDef {
                name: "fetch_html",
                description: "Return the inner HTML of a DOM element by node_id",
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "node_id": { "type": "integer", "description": "node_id from find_node (omit for full document body)" }
                    }
                }),
            },
        },
        Tool {
            r#type: "function",
            function: ToolDef {
                name: "fetch_text",
                description: "Return the visible text content of a DOM element by node_id",
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "node_id": { "type": "integer" }
                    },
                    "required": ["node_id"]
                }),
            },
        },
        // Tool {
        //     r#type: "function",
        //     function: ToolDef {
        //         name: "eval_js",
        //         description: "Execute JavaScript in the browser and return the result",
        //         parameters: json!({
        //             "type": "object",
        //             "properties": {
        //                 "script": { "type": "string" }
        //             },
        //             "required": ["script"]
        //         }),
        //     },
        // },
        Tool {
            r#type: "function",
            function: ToolDef {
                name: "wait",
                description: "Pauses execution for the given number of milliseconds. Use when the page needs time to load or react before proceeding.",
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "ms": { "type": "integer" }
                    },
                    "required": ["ms"]
                }),
            },
        },
        // TODO: mouse_move should be conditionally registered only when the target is a desktop browser.
        // For smartphone/touch-emulated sessions, use scroll_by/scroll_to and tap actions instead.
        Tool {
            r#type: "function",
            function: ToolDef {
                name: "mouse_move",
                description: "Move the mouse pointer to pixel coordinates without clicking",
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
                name: "get_ui_map",
                description: "Get the accessible UI element tree of the page. Each node has an id (use directly for node operations), role, name, and optional value/properties.",
                parameters: json!({ "type": "object", "properties": {} }),
            },
        },
        Tool {
            r#type: "function",
            function: ToolDef {
                name: "screenshot",
                description: "Capture a screenshot to see the current browser state. Use sparingly — only when get_ui_map is insufficient and you are stuck.",
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
    NodeClick { node_id: i64, human: bool },
    Type { text: String, node_id: Option<i64> },
    MouseMove { x: f32, y: f32 },
    HumanMouseMove { x: f32, y: f32 },
    ScrollBy { y: i32, human: bool },
    ScrollTo { node_id: i64, human: bool },
    FetchHtml { node_id: Option<i64> },
    FetchText { node_id: i64 },
    EvalJs { script: String },
    FindNode { selector: String },
    WaitForNode { selector: String, timeout_ms: u64 },
    Wait { ms: u64 },
    Screenshot,
    GetUiMap,
    /// keys: "Backspace", "Backspace10", "Backspace, Backspace, Backspace"
    SendKeys { keys: String },
    /// key: "Backspace3000" — key name followed by duration in ms
    HoldKey { key: String },
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
        // mouse_move exposed as human_mouse_move — always natural movement
        "mouse_move" => {
            v["type"] = json!("human_mouse_move");
            return serde_json::from_value(v)
                .map_err(|e| AIError::InvalidResponse(format!("mouse_move: {e}")));
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

#[derive(Debug, Deserialize)]
pub(crate) struct AIChatResponse {
    pub choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Choice {
    pub message: ResponseMessage,
}

#[derive(Debug, Deserialize)]
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
