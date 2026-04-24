use criterion::{Criterion, black_box, criterion_group, criterion_main};
use rusty_common::ai::{Message, ToolCall, ToolCallFunction, browser_tools, parse_action};

fn make_call(name: &str, args: &str) -> ToolCall {
    ToolCall {
        id: "call-bench".to_string(),
        r#type: "function".to_string(),
        function: ToolCallFunction {
            name: name.to_string(),
            arguments: args.to_string(),
        },
    }
}

fn bench_parse_action_navigate(c: &mut Criterion) {
    let call = make_call("navigate", r#"{"url":"https://example.com/path?q=1"}"#);
    c.bench_function("ai/parse_action/navigate", |b| {
        b.iter(|| parse_action(black_box(&call)));
    });
}

fn bench_parse_action_click(c: &mut Criterion) {
    // click injects human=true — exercises the mutation path
    let call = make_call("click", r#"{"x":123.5,"y":456.0}"#);
    c.bench_function("ai/parse_action/click", |b| {
        b.iter(|| parse_action(black_box(&call)));
    });
}

fn bench_parse_action_mouse_move(c: &mut Criterion) {
    // mouse_move rewrites type → human_mouse_move
    let call = make_call("mouse_move", r#"{"x":50.0,"y":75.0}"#);
    c.bench_function("ai/parse_action/mouse_move", |b| {
        b.iter(|| parse_action(black_box(&call)));
    });
}

fn bench_parse_action_engage_input(c: &mut Criterion) {
    let call = make_call("engage_input", r#"{"node_id":42,"value":"hello world"}"#);
    c.bench_function("ai/parse_action/engage_input", |b| {
        b.iter(|| parse_action(black_box(&call)));
    });
}

fn bench_parse_action_done(c: &mut Criterion) {
    let call = make_call("done", r#"{"result":"task completed successfully"}"#);
    c.bench_function("ai/parse_action/done", |b| {
        b.iter(|| parse_action(black_box(&call)));
    });
}

fn bench_browser_tools(c: &mut Criterion) {
    c.bench_function("ai/browser_tools", |b| {
        b.iter(|| black_box(browser_tools()));
    });
}

fn bench_message_system(c: &mut Criterion) {
    c.bench_function("ai/message/system", |b| {
        b.iter(|| Message::system(black_box("You are a helpful browser automation assistant.")));
    });
}

fn bench_message_user(c: &mut Criterion) {
    c.bench_function("ai/message/user", |b| {
        b.iter(|| Message::user(black_box("Click the submit button on the form.")));
    });
}

fn bench_message_user_with_screenshot(c: &mut Criterion) {
    // Simulate a realistic base64 payload size (~1KB)
    let screenshot = "A".repeat(1024);
    c.bench_function("ai/message/user_with_screenshot", |b| {
        b.iter(|| {
            Message::user_with_screenshot(
                black_box("Click the login button"),
                black_box(&screenshot),
            )
        });
    });
}

fn bench_message_tool_result(c: &mut Criterion) {
    c.bench_function("ai/message/tool_result", |b| {
        b.iter(|| Message::tool_result(black_box("call-abc123"), black_box("success")));
    });
}

fn bench_parse_action_invalid_json(c: &mut Criterion) {
    let call = make_call("navigate", "not json at all }{");
    c.bench_function("ai/parse_action/invalid_json", |b| {
        b.iter(|| parse_action(black_box(&call)));
    });
}

criterion_group!(
    benches,
    bench_parse_action_navigate,
    bench_parse_action_click,
    bench_parse_action_mouse_move,
    bench_parse_action_engage_input,
    bench_parse_action_done,
    bench_browser_tools,
    bench_message_system,
    bench_message_user,
    bench_message_user_with_screenshot,
    bench_message_tool_result,
    bench_parse_action_invalid_json,
);
criterion_main!(benches);
