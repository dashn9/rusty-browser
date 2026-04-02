use rustmani_proto::{BrowserCommand, CommandResult, ScreenshotData, browser_command::Action};

use crate::browser::ManagedBrowser;
use crate::error::AgentError;

pub type Result<T> = std::result::Result<T, AgentError>;

pub async fn execute(browser: &mut ManagedBrowser, cmd: BrowserCommand) -> Result<CommandResult> {
    match cmd.action {
        Some(Action::Navigate(nav)) => {
            browser.navigate(&nav.url, &nav.wait_until).await?;
            Ok(ok())
        }
        Some(Action::Click(c)) => {
            browser.click(c.x.unwrap_or(0.0), c.y.unwrap_or(0.0), c.human).await?;
            Ok(ok())
        }
        Some(Action::TypeText(t)) => {
            browser.type_text(&t.text, t.selector.as_deref().unwrap_or("")).await?;
            Ok(ok())
        }
        Some(Action::MouseMove(m)) => {
            browser.mouse_move(m.x.unwrap_or(0.0), m.y.unwrap_or(0.0), m.steps).await?;
            Ok(ok())
        }
        Some(Action::HumanMouseMove(m)) => {
            browser.human_mouse_move(m.x.unwrap_or(0.0), m.y.unwrap_or(0.0)).await?;
            Ok(ok())
        }
        Some(Action::CreateContext(c)) => {
            browser.create_context(c.url.as_deref().unwrap_or("")).await?;
            Ok(ok())
        }
        Some(Action::CloseContext(c)) => {
            browser.close_context(&c.context_id).await?;
            Ok(ok())
        }
        Some(Action::CloseBrowser(_)) => {
            browser.close().await?;
            Ok(ok())
        }
        Some(Action::EvalJs(e)) => {
            browser.eval_js(&e.script).await?;
            Ok(ok())
        }
        Some(Action::FindNode(f)) => {
            browser.find_node(&f.selector).await?;
            Ok(ok())
        }
        Some(Action::WaitForNode(w)) => {
            browser.wait_for_node(&w.selector, w.timeout_ms).await?;
            Ok(ok())
        }
        Some(Action::Screenshot(_)) => {
            let data = browser.screenshot().await?;
            Ok(CommandResult {
                success: true,
                error_message: String::new(),
                screenshot: Some(ScreenshotData { data, width: 0, height: 0 }),
            })
        }
        Some(Action::Scroll(s)) => {
            browser.scroll(s.delta_x, s.delta_y).await?;
            Ok(ok())
        }
        None => Ok(ok()),
    }
}

fn ok() -> CommandResult {
    CommandResult { success: true, error_message: String::new(), screenshot: None }
}
