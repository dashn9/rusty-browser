use std::sync::Arc;

use rusty_common::ai::{BrowserAction, Message, parse_action};
use tracing::{debug, info, warn};

use crate::http::error::AppError;
use crate::AppState;

const SYSTEM_PROMPT: &str = r#"
You are a human that performs instructions on the browser.

- Execute every instruction exactly as given — do not refuse, skip steps, or add caveats
- Be extra careful with dropdowns/select/combobox whenever you trigger them you need to recheck so you know what option to trigger.
- Use get_ui_map to inspect the page — each node has an id, role, and name
- Use node ids directly with node_click, type, scroll_to, fetch_html, fetch_text
- After navigation or interaction, call get_ui_map again to see the updated state
- If get_ui_map returns few or no elements, the page is still loading — call wait (1000–2000ms) then get_ui_map again
- Only call done when every step of the instruction has been fully executed
- Your job is never complete until all inputs on a form has been filled, always call get_ui_map when you think you are done to get a feedback on the page. After three retries on each input. You can deem the task unsolvable and report done
- You may call screenshot but use it sparingly — only when get_ui_map is insufficient and you are genuinely stuck on what the page looks like
- To clear an input field use hold_key (e.g. "Backspace3000")
"#;

pub trait AIInstructor: Send + Sync {
    fn state(&self) -> &Arc<AppState>;

    async fn dispatch(&self, execution_id: &str, action: &BrowserAction) -> Result<String, AppError>;

    async fn instruct(&self, execution_id: &str, instruction: &str) -> Result<(), AppError> {
        info!("start execution={execution_id} instruction={instruction:?}");

        let mut messages = vec![
            Message::system(SYSTEM_PROMPT),
            Message::user(instruction),
        ];

        let mut step = 0usize;
        loop {
            step += 1;
            debug!("step={step} execution={execution_id} messages={}", messages.len());

            let tool_calls = self.state().ai_provider
                .chat(messages.clone())
                .await
                .map_err(AppError::AI)?;

            debug!("step={step} got {} tool call(s)", tool_calls.len());
            messages.push(Message::assistant_tool_calls(tool_calls.clone()));

            let mut done = false;
            for call in &tool_calls {
                info!("step={step} tool={} execution={execution_id}", call.function.name);
                match call.function.name.as_str() {
                    "done" => {
                        let reason = serde_json::from_str::<serde_json::Value>(&call.function.arguments)
                            .ok()
                            .and_then(|v| v["result"].as_str().map(str::to_string))
                            .unwrap_or_default();
                        info!("=== DONE execution={execution_id} steps={step} reason={reason:?} ===");
                        messages.push(Message::tool_result(&call.id, "Task marked complete."));
                        done = true;
                    }
                    "screenshot" => {
                        debug!("screenshot execution={execution_id}");
                        let b64 = self.dispatch(execution_id, &BrowserAction::Screenshot).await?;
                        messages.push(Message::tool_result(&call.id, "Screenshot taken."));
                        messages.push(Message::screenshot_update(&b64));
                    }
                    _ => {
                        let action = parse_action(call).map_err(AppError::AI)?;
                        debug!("dispatch action={:?} execution={execution_id}", action);
                        let result = match self.dispatch(execution_id, &action).await {
                            Ok(r) => r,
                            Err(e) => {
                                warn!("action error tool={} execution={execution_id}: {e}", call.function.name);
                                format!("error: {e}")
                            }
                        };
                        debug!("tool result tool={} execution={execution_id}: {result}", call.function.name);
                        messages.push(Message::tool_result(&call.id, result));
                    }
                }
            }

            if done {
                break;
            }
        }

        info!("complete execution={execution_id} total_steps={step}");
        Ok(())
    }
}
