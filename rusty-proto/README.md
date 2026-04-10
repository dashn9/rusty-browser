# rusty-proto

Protobuf definitions and generated Rust bindings for the `BrowserAgent` gRPC service.

## Service

```proto
service BrowserAgent {
  rpc Execute(BrowserCommand) returns (CommandResult);
}
```

The Master connects to each browser agent over TLS and calls `Execute` with a `BrowserCommand`. The agent runs the command on the browser and returns a `CommandResult`.

`CommandResult.screenshot` is only populated for screenshot commands. All other commands return `success` / `error_message` only.

## Key Messages

| Message | Description |
|---|---|
| `BrowserCommand` | Command sent from Master to agent (navigate, click, type, screenshot, etc.) |
| `CommandResult` | Result returned by the agent |
| `ScreenshotData` | Screenshot bytes + dimensions, present only on screenshot results |
