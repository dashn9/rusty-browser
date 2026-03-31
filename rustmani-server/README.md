# rustmani-server

The master server for the rustmani browser automation cluster. Exposes an HTTP API to manage browser agents and an AI instruct engine to drive them.

## Overview

- Spawns browser agents on demand via [Flux](../rustmani-agent)
- Maintains a registry of active agents (host, gRPC port, contexts, status) in Redis
- Communicates with each browser agent directly over gRPC using its registered connection info
- Runs an AI instruct engine for screenshot-based browser automation

## Architecture

```
Client
  │
  ▼ HTTP API
rustmani-server
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
| `POST` | `/browsers` | Spawn a new browser agent |
| `GET` | `/browsers` | List all registered browsers |
| `GET` | `/browsers/:id` | Get a browser by ID |
| `DELETE` | `/browsers/:id` | Close and deregister a browser |
| `POST` | `/browsers/:id/contexts` | Create a context on a browser |
| `DELETE` | `/browsers/:id/contexts/:ctx_id` | Close a context |
| `POST` | `/instruct` | Run an AI instruct task on a browser |

## Configuration

Set via `rustmani.yaml` (path overridable with `RUSTMANI_CONFIG` env var):

```yaml
server:
  http_port: 8080
  grpc_port: 50051

redis:
  url: redis://localhost:6379
  key_prefix: rustmani

flux:
  base_url: http://localhost:8090
  api_key: ...
  function_name: rustmani-agent

ai:
  provider: ...

min_browsers: 2
```

## Running

```sh
RUSTMANI_CONFIG=rustmani.yaml cargo run --bin rustmani
```
