use std::sync::Arc;

use rusty_common::ai::{BrowserAction, Message, parse_action};
use tracing::{debug, info, warn};

use crate::http::error::AppError;
use crate::AppState;

const SYSTEM_PROMPT: &str = "You are a browser automation agent. You receive a screenshot of the current browser state and an instruction. Use the provided tools to complete the instruction step by step. Call 'screenshot' any time you need to see the updated state. When the task is complete, call 'done' with a summary of the outcome.";

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
                        info!("done execution={execution_id} steps={step}");
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
