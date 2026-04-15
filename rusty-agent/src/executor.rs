use rusty_proto::{BrowserCommand, CommandResult, browser_command::Action};

use crate::browser::ManagedBrowser;
use crate::error::AgentError;

pub type Result<T> = std::result::Result<T, AgentError>;

pub async fn execute(browser: &mut ManagedBrowser, cmd: BrowserCommand) -> Result<CommandResult> {
    tracing::info!("execute: {:?}", cmd.action);
    match cmd.action {
        Some(Action::Navigate(nav)) => {
            browser.navigate(&nav.url, &nav.wait_until).await?;
            Ok(ok())
        }
        Some(Action::Click(c)) => {
            browser.click(c.x.unwrap_or(0.0), c.y.unwrap_or(0.0), c.human).await?;
            Ok(ok())
        }
        Some(Action::NodeClick(c)) => {
            browser.node_click(&c.selector, c.human).await?;
            Ok(ok())
        }
        Some(Action::TypeText(t)) => {
            browser.type_text(t.text, t.selector).await?;
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
            // Handled directly in server.rs before reaching execute() —
            // close() consumes ManagedBrowser by value, which isn't possible
            // here since execute() only holds &mut. The server takes ownership
            // via Option::take(), calls close(), then exits.
            std::process::exit(0);
        }
        Some(Action::EvalJs(e)) => {
            let result = browser.eval_js(&e.script).await?;
            Ok(ok_with(result))
        }
        Some(Action::FindNode(f)) => {
            let found = browser.find_node(&f.selector).await?;
            Ok(ok_with(found.to_string()))
        }
        Some(Action::WaitForNode(w)) => {
            let found = browser.wait_for_node(&w.selector, w.timeout_ms).await?;
            Ok(ok_with(found.to_string()))
        }
        Some(Action::Screenshot(sc)) => {
            let b64 = browser.screenshot(sc.quality, &sc.format).await?;
            Ok(ok_with(b64))
        }
        Some(Action::ScrollBy(s)) => {
            browser.scroll_by(s.y, s.human).await?;
            Ok(ok())
        }
        Some(Action::ScrollTo(s)) => {
            browser.scroll_to(&s.selector, s.human, s.to).await?;
            Ok(ok())
        }
        Some(Action::FetchHtml(f)) => {
            let html = browser.fetch_html(f.selector.as_deref()).await?;
            Ok(ok_with(html))
        }
        Some(Action::FetchText(f)) => {
            let text = browser.fetch_text(&f.selector).await?;
            Ok(ok_with(text))
        }
        None => Ok(ok()),
    }
}

fn ok() -> CommandResult {
    CommandResult { success: true, error_message: String::new(), result: String::new() }
}

fn ok_with(result: String) -> CommandResult {
    CommandResult { success: true, error_message: String::new(), result }
}
