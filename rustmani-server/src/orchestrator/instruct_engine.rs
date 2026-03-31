use std::sync::Arc;

use tracing::info;

use rustmani_common::ai::{AIContent, AIMessage, BrowserAction};
use rustmani_common::error::RustmaniError;

use crate::AppState;

pub async fn run(
    state: &Arc<AppState>,
    browser_id: &str,
    instruction: &str,
    max_steps: u32,
) -> Result<(), RustmaniError> {
    let browser = state.redis.get_browser(browser_id).await?;
    let addr = format!("https://{}:{}", browser.host, browser.grpc_port);
    let mut client = rustmani_proto::browser_agent_client::BrowserAgentClient::connect(addr)
        .await
        .map_err(|e| RustmaniError::Internal(format!("Failed to connect to browser agent: {e}")))?;

    let mut history: Vec<AIMessage> = Vec::new();

    for step in 0..max_steps {
        info!("Instruct step {}/{} for browser {}", step + 1, max_steps, browser_id);

        state.redis.set_instruct_state(browser_id, "running", step + 1, max_steps, instruction, "").await?;

        let result = client
            .execute(tonic::Request::new(rustmani_proto::BrowserCommand {
                browser_id: browser_id.to_string(),
                context_id: String::new(),
                action: Some(rustmani_proto::browser_command::Action::Screenshot(
                    rustmani_proto::Screenshot {
                        quality: state.config.ai.resolution.quality,
                        format: state.config.ai.resolution.format.clone(),
                    },
                )),
            }))
            .await
            .map_err(|e| RustmaniError::Internal(format!("Screenshot failed: {e}")))?
            .into_inner();

        let screenshot_data = result.screenshot
            .ok_or_else(|| RustmaniError::Internal("No screenshot data in response".into()))?;

        let processed = downscale(&screenshot_data.data, state.config.ai.resolution.max_width, state.config.ai.resolution.quality)?;
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
    client: &mut rustmani_proto::browser_agent_client::BrowserAgentClient<tonic::transport::Channel>,
    browser_id: &str,
    action: &BrowserAction,
) -> Result<(), RustmaniError> {
    let proto_action = match action {
        BrowserAction::Navigate { url } => rustmani_proto::browser_command::Action::Navigate(
            rustmani_proto::Navigate { url: url.clone(), wait_until: "complete".to_string() },
        ),
        BrowserAction::Click { x, y, human } => rustmani_proto::browser_command::Action::Click(
            rustmani_proto::Click { selector: None, x: Some(*x), y: Some(*y), human: *human },
        ),
        BrowserAction::Type { text, selector } => rustmani_proto::browser_command::Action::TypeText(
            rustmani_proto::Type { text: text.clone(), selector: selector.clone() },
        ),
        BrowserAction::MouseMove { x, y } => rustmani_proto::browser_command::Action::HumanMouseMove(
            rustmani_proto::HumanMouseMove { selector: None, x: Some(*x), y: Some(*y) },
        ),
        BrowserAction::Scroll { delta_x, delta_y } => rustmani_proto::browser_command::Action::Scroll(
            rustmani_proto::Scroll { delta_x: *delta_x, delta_y: *delta_y },
        ),
        BrowserAction::Wait { ms } => {
            tokio::time::sleep(std::time::Duration::from_millis(*ms)).await;
            return Ok(());
        }
        BrowserAction::Screenshot | BrowserAction::Done { .. } => return Ok(()),
    };

    client.execute(tonic::Request::new(rustmani_proto::BrowserCommand {
        browser_id: browser_id.to_string(),
        context_id: String::new(),
        action: Some(proto_action),
    }))
    .await
    .map_err(|e| RustmaniError::Internal(format!("Action failed: {e}")))?;

    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    Ok(())
}

fn downscale(data: &[u8], max_width: u32, quality: u32) -> Result<Vec<u8>, RustmaniError> {
    if data.is_empty() { return Ok(data.to_vec()); }
    let img = image::load_from_memory(data)
        .map_err(|e| RustmaniError::Internal(format!("Failed to decode screenshot: {e}")))?;
    let img = if img.width() > max_width {
        let ratio = max_width as f32 / img.width() as f32;
        img.resize(max_width, (img.height() as f32 * ratio) as u32, image::imageops::FilterType::Lanczos3)
    } else {
        img
    };
    let mut buf = std::io::Cursor::new(Vec::new());
    img.write_with_encoder(image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, quality as u8))
        .map_err(|e| RustmaniError::Internal(format!("Failed to encode screenshot: {e}")))?;
    Ok(buf.into_inner())
}
