use std::sync::Arc;

use tracing::info;

use rusty_common::ai::{AIContent, AIMessage, BrowserAction};

use crate::http::error::AppError;
use crate::AppState;

pub async fn run(
    state: &Arc<AppState>,
    browser_id: &str,
    instruction: &str,
    max_steps: u32,
) -> Result<(), AppError> {
    let browser = state.redis.get_browser(browser_id).await?
        .ok_or_else(|| AppError::NotFound(browser_id.to_string()))?;
    let addr = format!("https://{}:{}", browser.host, browser.grpc_port);
    let mut client = rusty_proto::browser_agent_client::BrowserAgentClient::connect(addr)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to connect to browser agent: {e}")))?;

    let mut history: Vec<AIMessage> = Vec::new();

    for step in 0..max_steps {
        info!("Instruct step {}/{} for browser {}", step + 1, max_steps, browser_id);

        state.redis.set_instruct_state(browser_id, "running", step + 1, max_steps, instruction, "").await?;

        let result = client
            .execute(tonic::Request::new(rusty_proto::BrowserCommand {
                browser_id: browser_id.to_string(),
                context_id: String::new(),
                action: Some(rusty_proto::browser_command::Action::Screenshot(
                    rusty_proto::Screenshot::default(),
                )),
            }))
            .await
            .map_err(|e| AppError::Internal(format!("Screenshot failed: {e}")))?
            .into_inner();

        let screenshot_data = result.screenshot
            .ok_or_else(|| AppError::Internal("No screenshot data in response".into()))?;

        let processed = encode_screenshot(&screenshot_data.data, state.config.ai.resolution.quality)?;
        let screenshot_b64 = base64::engine::general_purpose::STANDARD.encode(&processed);

        let ai_response = state.ai_provider.analyze_screenshot(&screenshot_b64, instruction, &history).await?;

        info!("AI step {}: reasoning={}, done={}", step + 1, ai_response.reasoning, ai_response.done);

        state.redis.set_instruct_state(
            browser_id,
            if ai_response.done { "completed" } else { "running" },
            step + 1, max_steps, instruction, &ai_response.reasoning,
        ).await?;

        history.push(AIMessage {
            role: "user".to_string(),
            content: AIContent::ImageAndText {
                image_base64: screenshot_b64,
                text: format!("Step {}", step + 1),
            },
        });
        history.push(AIMessage {
            role: "assistant".to_string(),
            content: AIContent::Text(serde_json::to_string(&ai_response).unwrap_or_default()),
        });

        state.redis.push_instruct_history(browser_id, &serde_json::to_string(&ai_response).unwrap_or_default()).await?;

        if ai_response.done {
            return Ok(());
        }

        for action in &ai_response.actions {
            dispatch_action(&mut client, browser_id, action).await?;
        }

        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }

    state.redis.set_instruct_state(browser_id, "completed", max_steps, max_steps, instruction, "Max steps reached").await?;
    Ok(())
}

async fn dispatch_action(
    client: &mut rusty_proto::browser_agent_client::BrowserAgentClient<tonic::transport::Channel>,
    browser_id: &str,
    action: &BrowserAction,
) -> Result<(), AppError> {
    let proto_action = match action {
        BrowserAction::Navigate { url } => rusty_proto::browser_command::Action::Navigate(
            rusty_proto::Navigate { url: url.clone(), wait_until: "complete".to_string() },
        ),
        BrowserAction::Click { x, y, human } => rusty_proto::browser_command::Action::Click(
            rusty_proto::Click { selector: None, x: Some(*x), y: Some(*y), human: *human },
        ),
        BrowserAction::Type { text, selector } => rusty_proto::browser_command::Action::TypeText(
            rusty_proto::Type { text: text.clone(), selector: selector.clone() },
        ),
        BrowserAction::MouseMove { x, y } => rusty_proto::browser_command::Action::HumanMouseMove(
            rusty_proto::HumanMouseMove { selector: None, x: Some(*x), y: Some(*y) },
        ),
        BrowserAction::Scroll { delta_x, delta_y } => rusty_proto::browser_command::Action::Scroll(
            rusty_proto::Scroll { delta_x: *delta_x, delta_y: *delta_y },
        ),
        BrowserAction::Wait { ms } => {
            tokio::time::sleep(std::time::Duration::from_millis(*ms)).await;
            return Ok(());
        }
        BrowserAction::Screenshot | BrowserAction::Done { .. } => return Ok(()),
    };

    client.execute(tonic::Request::new(rusty_proto::BrowserCommand {
        browser_id: browser_id.to_string(),
        context_id: String::new(),
        action: Some(proto_action),
    }))
    .await
    .map_err(|e| AppError::Internal(format!("Action failed: {e}")))?;

    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    Ok(())
}

fn encode_screenshot(data: &[u8], quality: u32) -> Result<Vec<u8>, AppError> {
    if data.is_empty() { return Ok(data.to_vec()); }
    let img = image::load_from_memory(data)
        .map_err(|e| AppError::Internal(format!("Failed to decode screenshot: {e}")))?;
    let mut buf = std::io::Cursor::new(Vec::new());
    img.write_with_encoder(image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, quality as u8))
        .map_err(|e| AppError::Internal(format!("Failed to encode screenshot: {e}")))?;
    Ok(buf.into_inner())
}
