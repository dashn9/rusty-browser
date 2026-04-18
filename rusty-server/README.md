# rusty-server

The master server for the rusty browser automation cluster. Exposes an HTTP API to manage browser agents and an AI instruct engine to drive them.

## Overview

- Spawns browser agents on demand via [Flux](../rusty-agent)
- Maintains a registry of active agents (host, gRPC port, contexts, status) in Redis
- Communicates with each browser agent directly over gRPC using its registered connection info
- Runs an AI instruct engine for screenshot-based browser automation

## Architecture

```
Client
  │
  ▼ HTTP API
rusty-server
  ├── Redis (browser registry)
  ├── Flux (spawn browser agents)
  └── gRPC client ──► browser-agent-1 (rustenium)
                  ──► browser-agent-2 (rustenium)
                  ──► ...
```

Each browser agent owns exactly one browser instance. When spawned, Flux returns the agent's `id`, `host`, and `grpc_port` which the server registers and uses for all subsequent commands.

## HTTP API

| Method | Path | Description |
|--------|------|-------------|
| `PUT` | `/browsers/` | Spawn a new browser agent |
| `GET` | `/browsers/` | List all registered browsers |
| `DELETE` | `/browsers/` | Delete all browsers |
| `DELETE` | `/teardown/` | Delete all browsers and terminate all Flux nodes |
| `GET` | `/browsers/{id}/` | Get a browser by ID |
| `DELETE` | `/browsers/{id}/` | Close and deregister a browser |
| `PUT` | `/browsers/{id}/contexts/` | Create a context on a browser |
| `DELETE` | `/browsers/{id}/contexts/{ctx_id}/` | Close a context |
| `POST` | `/browsers/{id}/navigate/` | Navigate to a URL |
| `POST` | `/browsers/{id}/click/` | Click at coordinates |
| `POST` | `/browsers/{id}/node-click/` | Click a DOM node by node_id |
| `POST` | `/browsers/{id}/type/` | Type text (optionally into a node_id) |
| `POST` | `/browsers/{id}/scroll-by/` | Scroll by Y pixels |
| `POST` | `/browsers/{id}/scroll-to/` | Scroll a node_id into view |
| `POST` | `/browsers/{id}/screenshot/` | Capture a base64 JPEG screenshot |
| `POST` | `/browsers/{id}/fetch-html/` | Fetch inner HTML of a node_id (or full page) |
| `POST` | `/browsers/{id}/fetch-text/` | Fetch inner text of a node_id |
| `POST` | `/browsers/{id}/find-node/` | Find a node by CSS selector, returns node_id |
| `POST` | `/browsers/{id}/wait-for-node/` | Wait for a CSS selector, returns node_id |
| `GET` | `/browsers/{id}/ui-map/` | Get accessible UI node tree |
| `POST` | `/browsers/{id}/eval/` | Evaluate JavaScript |
| `POST` | `/browsers/{id}/instruct/` | Run a natural language instruction |
| `GET` | `/browsers/{id}/logs/` | Fetch execution logs from Flux |

## Configuration

Set via `rusty.yaml` (path overridable with `RUSTY_CONFIG` env var). See [`example.rusty.yaml`](example.rusty.yaml) for a full annotated reference.

### Local development (no Flux)

Set `flux.local_binary` to spawn agents as local subprocesses instead of deploying via Flux:

```yaml
flux:
  local_binary: "cargo run -p rusty-agent --"
```

Agent stdout/stderr is forwarded through the server's tracing output under the `rusty_agent` target.

## Running

```sh
RUSTY_CONFIG=rusty.yaml cargo run --bin rusty
```

## Release & Deployments

Deployments are strictly manual and orchestrated via the GitHub Actions **Release Deploy** workflow. 

To ship a new release:
1. Update the `version = "X.X.X"` inside `rusty-server/Cargo.toml` (and other agent/common crates if preferred).
2. Commit and push the changes.
3. Trigger the `Release Deploy` action from the **Actions** tab on GitHub using `workflow_dispatch`.

The workflow parses the version straight from `Cargo.toml`. To prevent mistakenly overwriting stable endpoints, the action will **reject and abort** if a GitHub Release for that version already exists. If you *really* need to overwrite a botched release, check the **"force overwrite"** box when triggering the workflow.
