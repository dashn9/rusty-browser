# rustmani-agent

Browser agent deployed via Flux. Each instance owns exactly one browser and exposes a gRPC server (TLS) for the Master to connect to and send commands.

## How it works

1. Flux spawns the agent — returning its `browser_id`, `host`, and `grpc_port` to the Master
2. Agent launches the browser (with optional identity/fingerprint via `rustenium-identity`)
3. Agent binds a gRPC server on a free OS-assigned port and prints connection info to stdout
4. Master uses that info to connect directly over TLS and send `BrowserCommand` messages

## gRPC Service

```proto
service BrowserAgent {
  rpc Execute(BrowserCommand) returns (CommandResult);
}
```

Screenshot commands populate `CommandResult.screenshot`. All other commands return `success`/`error_message` only.

## TLS

The agent loads its certificate and key from `tls/agent.crt` and `tls/agent.key` at startup.

## Environment Variables

| Variable | Description |
|---|---|
| `RUSTMANI_BROWSER_ID` | Browser ID assigned by the Master via Flux |
| `RUSTMANI_AGENT_HOST` | Hostname/IP reported back to the Master (default: `127.0.0.1`) |
| `RUSTMANI_IDENTITY_JSON` | Optional identity/fingerprint JSON for `rustenium-identity` |
