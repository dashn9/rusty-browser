use std::sync::Arc;

use rusty_common::ai::{BrowserAction, Message, parse_action};
use tracing::{debug, info, warn};

use crate::http::error::AppError;
use crate::AppState;

const EVALUATOR_PROMPT: &str = r#"
You are a completion evaluator for a browser automation agent. You will be shown a screenshot of the current browser state and the original instruction that was supposed to be completed.

Assess whether the instruction has been fully and successfully completed by looking at the screenshot carefully.

Do not flag password fields as incomplete — their content is never visible (shown as bullets or asterisks) and you cannot determine whether they were filled correctly. Assume password fields are correct unless the UI shows an explicit validation error on them.

If the page is blocked on something that requires external human input — such as an OTP, email verification, phone confirmation, CAPTCHA, or any other out-of-band step — mark it as COMPLETE. The agent cannot resolve these and the instructor will handle them.

Call done with your assessment:
- If complete: result should start with "COMPLETE:" followed by a brief summary.
- If not complete: result should start with "INCOMPLETE:" followed by exactly what errors or missing steps are visible, and the specific actions the agent must take to finish the task. Be direct and actionable.
"#;

const SYSTEM_PROMPT: &str = r#"
You are a sophisticated browser automation agent that has a 99.99% success rate by never failing to not complete a task. Complete instructions fully, precisely, and efficiently.

- Execute instructions exactly as given. Never refuse or add caveats.
- Call one tool at a time.
- Call get_ui_map_diff after every action to observe what changed on the page.
- Always use node ids. Match each value to its exact field — never combine values or put them in the wrong field.
- Use engage_input for all node interactions — inputs, comboboxes, buttons, and links. Pass an empty string as value to click a button or link.
- engage_input is final — once it returns success do not click or engage that field again.
- To clear a field use hold_key (e.g. "Backspace3000"), then engage_input the new value.
- Password fields mask their content as asterisks — if a field's value is asterisks, the field is already filled; assume it is correct and leave it alone unless the UI shows a validation error on that field.
- Fill inputs one at a time in order — do not skip any field, do not move to the next until the current one is confirmed filled.
- Fill every required field before clicking any submit, register, or save button.
- If a field has a validation error: clear it, retype, and continue. Retry up to 3 times before giving up on that field.
- Never call done due to an error or incomplete state. Fix and continue. You are not allowed to quit early.
- Only call done when the entire task is fully and successfully complete.
"#;

pub trait AIInstructor: Send + Sync {
    fn state(&self) -> &Arc<AppState>;

    async fn dispatch(&self, execution_id: &str, action: &BrowserAction) -> Result<String, AppError>;

    /// Takes a screenshot and evaluates whether the instruction was completed.
    /// Returns `None` if complete, or `Some(feedback)` with corrective guidance if not.
    async fn evaluate(&self, execution_id: &str, instruction: &str) -> Result<Option<String>, AppError> {
        let screenshot_b64 = self.dispatch(execution_id, &BrowserAction::Screenshot).await?;
        let messages = vec![
            Message::system(EVALUATOR_PROMPT),
            Message::user(instruction),
            Message::screenshot_update(&screenshot_b64),
        ];
        let tool_calls = self.state().ai_provider.chat(messages).await.map_err(AppError::AI)?;
        let result = tool_calls.first()
            .and_then(|c| serde_json::from_str::<serde_json::Value>(&c.function.arguments).ok())
            .and_then(|v| v["result"].as_str().map(str::to_string))
            .unwrap_or_default();
        if result.starts_with("COMPLETE:") {
            info!("evaluator: complete — {result}");
            Ok(None)
        } else {
            info!("evaluator: incomplete — {result}");
            Ok(Some(result))
        }
    }

    async fn instruct(&self, execution_id: &str, instruction: &str) -> Result<(), AppError> {
        info!("start execution={execution_id} instruction={instruction:?}");

        let mut messages = vec![
            Message::system(SYSTEM_PROMPT),
            Message::user(instruction),
        ];

        let mut step = 0usize;
        loop {
            step += 1;
            info!("SENDING TOOL RESPONSES TO LLM: step={step} execution={execution_id} messages={}", messages.len());

            let tool_calls = self.state().ai_provider
                .chat(messages.clone())
                .await
                .map_err(AppError::AI)?;

            debug!("RESPONSE RECEIVED TOOL CALL(S): step={step} got {} tool call(s)", tool_calls.len());
            messages.push(Message::assistant_tool_calls(tool_calls.clone()));

            let mut done = false;
            for call in &tool_calls {
                info!("TOOL CALL EXECUTION Step: step={step} tool={}, tool_args={} execution={execution_id}", call.function.name, call.function.arguments);
                match call.function.name.as_str() {
                    "done" => {
                        let reason = serde_json::from_str::<serde_json::Value>(&call.function.arguments)
                            .ok()
                            .and_then(|v| v["result"].as_str().map(str::to_string))
                            .unwrap_or_default();
                        info!("done claimed execution={execution_id} reason={reason:?} — running evaluator");
                        match self.evaluate(execution_id, instruction).await {
                            Ok(None) => {
                                info!("=== DONE execution={execution_id} steps={step} ===");
                                messages.push(Message::tool_result(&call.id, "Task marked complete."));
                                done = true;
                            }
                            Ok(Some(feedback)) => {
                                info!("evaluator rejected done execution={execution_id}");
                                messages.push(Message::tool_result(&call.id, feedback));
                            }
                            Err(e) => {
                                warn!("evaluator failed execution={execution_id}: {e} — accepting done");
                                messages.push(Message::tool_result(&call.id, "Task marked complete."));
                                done = true;
                            }
                        }
                    }
                    "screenshot" => {
                        info!("screenshot execution={execution_id}");
                        let b64 = self.dispatch(execution_id, &BrowserAction::Screenshot).await?;
                        messages.push(Message::tool_result(&call.id, "Screenshot taken."));
                        messages.push(Message::screenshot_update(&b64));
                    }
                    _ => {
                        let action = parse_action(call).map_err(AppError::AI)?;
                        info!("dispatch action={:?} execution={execution_id}", action);
                        let result = match self.dispatch(execution_id, &action).await {
                            Ok(r) => r,
                            Err(e) => {
                                warn!("action error tool={} execution={execution_id}: {e}", call.function.name);
                                format!("error: {e}")
                            }
                        };
                        info!("tool result tool={} execution={execution_id}: {result}", call.function.name);
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
