use std::sync::Arc;

use rusty_common::ai::{BrowserAction, Message, parse_action};

use crate::http::error::AppError;
use crate::AppState;

const SYSTEM_PROMPT: &str = "You are a browser automation agent. You receive a screenshot of the current browser state and an instruction. Use the provided tools to complete the instruction step by step. Call 'screenshot' any time you need to see the updated state. When the task is complete, call 'done' with a summary of the outcome.";

pub trait AIInstructor: Send + Sync {
    fn state(&self) -> &Arc<AppState>;

    async fn dispatch(&self, execution_id: &str, action: &BrowserAction) -> Result<String, AppError>;

    async fn instruct(&self, execution_id: &str, instruction: &str) -> Result<(), AppError> {
        // Seed the conversation with an initial screenshot
        let b64 = self.dispatch(execution_id, &BrowserAction::Screenshot).await?;

        let mut messages = vec![
            Message::system(SYSTEM_PROMPT),
            Message::user_with_screenshot(instruction, &b64),
        ];

        loop {
            let tool_calls = self.state().ai_provider
                .chat(messages.clone())
                .await
                .map_err(AppError::AI)?;

            messages.push(Message::assistant_tool_calls(tool_calls.clone()));

            let mut done = false;
            for call in &tool_calls {
                match call.function.name.as_str() {
                    "done" => {
                        messages.push(Message::tool_result(&call.id, "Task marked complete."));
                        done = true;
                    }
                    "screenshot" => {
                        let b64 = self.dispatch(execution_id, &BrowserAction::Screenshot).await?;
                        messages.push(Message::tool_result(&call.id, "Screenshot taken."));
                        messages.push(Message::screenshot_update(&b64));
                    }
                    _ => {
                        let action = parse_action(call).map_err(AppError::AI)?;
                        let result = match self.dispatch(execution_id, &action).await {
                            Ok(r) => r,
                            Err(e) => format!("error: {e}"),
                        };
                        messages.push(Message::tool_result(&call.id, result));
                    }
                }
            }

            if done {
                break;
            }
        }

        Ok(())
    }
}
