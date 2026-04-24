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
pub struct Tool {
    pub r#type: &'static str,
    pub function: ToolDef,
}

#[derive(Serialize)]
pub struct ToolDef {
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
        // Tool {
        //     r#type: "function",
        //     function: ToolDef {
        //         name: "node_click",
        //         description: "Click a DOM element by node_id returned from find_node or wait_for_node",
        //         parameters: json!({
        //             "type": "object",
        //             "properties": {
        //                 "node_id": { "type": "integer" }
        //             },
        //             "required": ["node_id"]
        //         }),
        //     },
        // },
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
        // Tool {
        //     r#type: "function",
        //     function: ToolDef {
        //         name: "get_ui_map",
        //         description: "Returns the full accessible UI element tree of the current page. Each node has an id, role, name, and optional value.",
        //         parameters: json!({ "type": "object", "properties": {} }),
        //     },
        // },
        Tool {
            r#type: "function",
            function: ToolDef {
                name: "get_ui_map_diff",
                description: "Returns only the nodes that changed or were added since the last get_ui_map call.",
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
                name: "engage_input",
                description: "Interact with any node — inputs, comboboxes, buttons, or links. For inputs: clicks then types. For comboboxes: opens and selects matching option. For buttons/links: pass empty string as value to just click.",
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "node_id": { "type": "integer" },
                        "value": { "type": "string", "description": "Text to type, option name to select, or empty string to just click" }
                    },
                    "required": ["node_id", "value"]
                }),
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
    GetUiMapDiff,
    /// keys: "Backspace", "Backspace10", "Backspace, Backspace, Backspace"
    SendKeys { keys: String },
    /// key: "Backspace3000" — key name followed by duration in ms
    HoldKey { key: String },
    Done { result: String },
    EngageInput { node_id: i64, value: String },
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AIProviderKind, ResolutionConfig};

    fn make_call(name: &str, args: &str) -> ToolCall {
        ToolCall {
            id: "call-1".to_string(),
            r#type: "function".to_string(),
            function: ToolCallFunction {
                name: name.to_string(),
                arguments: args.to_string(),
            },
        }
    }

    fn ai_config(provider: AIProviderKind) -> AIConfig {
        AIConfig {
            provider,
            api_key: "test-key".to_string(),
            model: "gpt-4o".to_string(),
            base_url: None,
            resolution: ResolutionConfig::default(),
        }
    }

    // ---- Message constructors ----

    #[test]
    fn message_system_role_and_content() {
        let m = Message::system("You are helpful.");
        assert_eq!(m.role, "system");
        assert_eq!(m.content.as_ref().unwrap().as_str().unwrap(), "You are helpful.");
        assert!(m.tool_calls.is_none());
        assert!(m.tool_call_id.is_none());
    }

    #[test]
    fn message_user_role_and_content() {
        let m = Message::user("Hello");
        assert_eq!(m.role, "user");
        assert_eq!(m.content.as_ref().unwrap().as_str().unwrap(), "Hello");
        assert!(m.tool_calls.is_none());
        assert!(m.tool_call_id.is_none());
    }

    #[test]
    fn message_user_with_screenshot_structure() {
        let m = Message::user_with_screenshot("click the button", "base64data==");
        assert_eq!(m.role, "user");
        let content = m.content.unwrap();
        let arr = content.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["type"].as_str().unwrap(), "text");
        assert!(arr[0]["text"].as_str().unwrap().contains("click the button"));
        assert_eq!(arr[1]["type"].as_str().unwrap(), "image_url");
        let url = arr[1]["image_url"]["url"].as_str().unwrap();
        assert!(url.starts_with("data:image/jpeg;base64,"));
        assert!(url.contains("base64data=="));
    }

    #[test]
    fn message_screenshot_update_structure() {
        let m = Message::screenshot_update("abc123");
        assert_eq!(m.role, "user");
        let content = m.content.unwrap();
        let arr = content.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["type"].as_str().unwrap(), "text");
        assert_eq!(arr[1]["type"].as_str().unwrap(), "image_url");
        let url = arr[1]["image_url"]["url"].as_str().unwrap();
        assert!(url.contains("abc123"));
    }

    #[test]
    fn message_assistant_tool_calls() {
        let calls = vec![make_call("navigate", r#"{"url":"https://example.com"}"#)];
        let m = Message::assistant_tool_calls(calls.clone());
        assert_eq!(m.role, "assistant");
        assert!(m.content.is_none());
        let stored = m.tool_calls.unwrap();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].function.name, "navigate");
    }

    #[test]
    fn message_tool_result_fields() {
        let m = Message::tool_result("call-xyz", "success");
        assert_eq!(m.role, "tool");
        assert_eq!(m.content.as_ref().unwrap().as_str().unwrap(), "success");
        assert_eq!(m.tool_call_id.as_deref(), Some("call-xyz"));
        assert!(m.tool_calls.is_none());
    }

    // ---- parse_action ----

    #[test]
    fn parse_action_navigate() {
        let call = make_call("navigate", r#"{"url":"https://example.com"}"#);
        let action = parse_action(&call).unwrap();
        assert!(matches!(action, BrowserAction::Navigate { url } if url == "https://example.com"));
    }

    #[test]
    fn parse_action_click_injects_human_true() {
        let call = make_call("click", r#"{"x":100.0,"y":200.0}"#);
        let action = parse_action(&call).unwrap();
        assert!(matches!(action, BrowserAction::Click { x, y, human: true } if x == 100.0 && y == 200.0));
    }

    #[test]
    fn parse_action_scroll_by_injects_human_true() {
        let call = make_call("scroll_by", r#"{"y":300}"#);
        let action = parse_action(&call).unwrap();
        assert!(matches!(action, BrowserAction::ScrollBy { y: 300, human: true }));
    }

    #[test]
    fn parse_action_scroll_to_injects_human_true() {
        let call = make_call("scroll_to", r#"{"node_id":42}"#);
        let action = parse_action(&call).unwrap();
        assert!(matches!(action, BrowserAction::ScrollTo { node_id: 42, human: true }));
    }

    #[test]
    fn parse_action_node_click_injects_human_true() {
        let call = make_call("node_click", r#"{"node_id":7}"#);
        let action = parse_action(&call).unwrap();
        assert!(matches!(action, BrowserAction::NodeClick { node_id: 7, human: true }));
    }

    #[test]
    fn parse_action_mouse_move_becomes_human_mouse_move() {
        let call = make_call("mouse_move", r#"{"x":50.0,"y":75.0}"#);
        let action = parse_action(&call).unwrap();
        assert!(matches!(action, BrowserAction::HumanMouseMove { x, y } if x == 50.0 && y == 75.0));
    }

    #[test]
    fn parse_action_type_text() {
        let call = make_call("type", r#"{"text":"hello world"}"#);
        let action = parse_action(&call).unwrap();
        assert!(matches!(action, BrowserAction::Type { text, node_id: None } if text == "hello world"));
    }

    #[test]
    fn parse_action_screenshot() {
        let call = make_call("screenshot", r#"{}"#);
        let action = parse_action(&call).unwrap();
        assert!(matches!(action, BrowserAction::Screenshot));
    }

    #[test]
    fn parse_action_done() {
        let call = make_call("done", r#"{"result":"task completed"}"#);
        let action = parse_action(&call).unwrap();
        assert!(matches!(action, BrowserAction::Done { result } if result == "task completed"));
    }

    #[test]
    fn parse_action_wait() {
        let call = make_call("wait", r#"{"ms":500}"#);
        let action = parse_action(&call).unwrap();
        assert!(matches!(action, BrowserAction::Wait { ms: 500 }));
    }

    #[test]
    fn parse_action_hold_key() {
        let call = make_call("hold_key", r#"{"key":"Backspace3000"}"#);
        let action = parse_action(&call).unwrap();
        assert!(matches!(action, BrowserAction::HoldKey { key } if key == "Backspace3000"));
    }

    #[test]
    fn parse_action_fetch_html_no_node_id() {
        let call = make_call("fetch_html", r#"{}"#);
        let action = parse_action(&call).unwrap();
        assert!(matches!(action, BrowserAction::FetchHtml { node_id: None }));
    }

    #[test]
    fn parse_action_fetch_text() {
        let call = make_call("fetch_text", r#"{"node_id":99}"#);
        let action = parse_action(&call).unwrap();
        assert!(matches!(action, BrowserAction::FetchText { node_id: 99 }));
    }

    #[test]
    fn parse_action_engage_input() {
        let call = make_call("engage_input", r#"{"node_id":5,"value":"hello"}"#);
        let action = parse_action(&call).unwrap();
        assert!(matches!(action, BrowserAction::EngageInput { node_id: 5, value } if value == "hello"));
    }

    #[test]
    fn parse_action_get_ui_map_diff() {
        let call = make_call("get_ui_map_diff", r#"{}"#);
        let action = parse_action(&call).unwrap();
        assert!(matches!(action, BrowserAction::GetUiMapDiff));
    }

    #[test]
    fn parse_action_invalid_json_returns_error() {
        let call = make_call("navigate", "not json at all");
        let err = parse_action(&call).unwrap_err();
        assert!(matches!(err, crate::error::AIError::InvalidResponse(_)));
    }

    #[test]
    fn parse_action_unknown_tool_returns_error() {
        let call = make_call("teleport", r#"{"destination":"mars"}"#);
        let err = parse_action(&call).unwrap_err();
        assert!(matches!(err, crate::error::AIError::InvalidResponse(_)));
    }

    // ---- browser_tools ----

    #[test]
    fn browser_tools_is_non_empty() {
        let tools = browser_tools();
        assert!(!tools.is_empty());
    }

    #[test]
    fn browser_tools_all_are_function_type() {
        for t in browser_tools() {
            assert_eq!(t.r#type, "function");
        }
    }

    #[test]
    fn browser_tools_contains_expected_names() {
        let tools = browser_tools();
        let names: Vec<&str> = tools.iter().map(|t| t.function.name).collect();
        for expected in ["navigate", "click", "type", "screenshot", "done", "engage_input"] {
            assert!(names.contains(&expected), "missing tool: {expected}");
        }
    }

    #[test]
    fn browser_tools_no_duplicate_names() {
        let tools = browser_tools();
        let mut names: Vec<&str> = tools.iter().map(|t| t.function.name).collect();
        let original_len = names.len();
        names.dedup();
        // after sort+dedup
        let mut sorted = tools.iter().map(|t| t.function.name).collect::<Vec<_>>();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), original_len, "duplicate tool names found");
    }

    // ---- create_provider ----

    #[test]
    fn create_provider_openai_does_not_panic() {
        let config = ai_config(AIProviderKind::OpenAI);
        let _provider = create_provider(&config);
    }

    #[test]
    fn create_provider_openrouter_does_not_panic() {
        let config = ai_config(AIProviderKind::OpenRouter);
        let _provider = create_provider(&config);
    }
}
