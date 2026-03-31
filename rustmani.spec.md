# Project Specification: rustmani

**Project Goal:** A high-concurrency browser automation orchestrator leveraging `serverless-flux` for elastic scaling, `rustenium` for human-emulated interaction, and direct gRPC communication between the Master and browser agents.

## 1. System Components

### rustmani-server (Master)
The central orchestrator and state authority. Provides a unified HTTP API secured by **API Key authentication**.
* **Browser Registry:** Tracks all active browser agents in Redis. Each entry stores the browser's `id`, `host`, `gRPC port`, `contexts`, and `state`.
* **Direct gRPC Client:** Connects to each browser agent using its registered host + gRPC port to send commands.
* **AI Instruct Engine:** Takes screenshots from browser agents, processes them (downscaling), sends to an AI provider, and dispatches the resulting actions back.

### rustmani-agent (Browser Agent)
The execution unit deployed via Flux. Each agent owns exactly one browser instance.
* **Startup:** Flux spawns the agent. The agent launches the browser and starts a gRPC server. Flux returns the agent's `browser_id`, `host`, and `gRPC port` to the Master, which registers it.
* **gRPC Server:** Exposes a `BrowserAgent` service. The Master connects directly to send `BrowserCommand` messages and receive `CommandResult` responses.
* **Driver Layer:** Uses `rustenium` for browser interactions and `rustenium-identity` for fingerprint/profile management.

### rustmani-cli
Developer interface for interacting with the cluster — creating browsers, running instruct tasks, and inspecting state.

### rustmani-common
Shared types, config, Redis store, AI provider interface, and Flux client used by both the server and agent.

### rustmani-proto
Protobuf definitions shared across the workspace.

---

## 2. State Definitions

Each browser agent in the registry has:
* **`id`** — Unique identifier assigned by the agent on startup.
* **`host` / `grpc_port`** — Connection info the Master uses to reach the agent.
* **`state`** — One of `Idle`, `Reserved`, `PartialReserved`.
* **`contexts`** — List of active context IDs within this browser instance (stored under `browser:{id}:contexts` in Redis).

---

## 3. Core Workflows

### Browser Creation
1. `POST /browsers` hits the Master.
2. Master calls `flux.execute_function(...)` to spawn a browser agent.
3. Flux returns `browser_id`, `host`, `grpc_port`.
4. Master stores the info in Redis and returns `browser_id` to the caller.

### Command Flow
1. Caller sends an HTTP request with a `browser_id`.
2. Master looks up `host` + `grpc_port` from Redis.
3. Master connects to the browser agent's gRPC endpoint.
4. `Execute(BrowserCommand)` is called — agent runs it on the browser and returns `CommandResult`.

### AI Instruct
1. `POST /browsers/{id}/instruct` with an instruction string.
2. Master connects to the browser agent, takes a screenshot via `Execute`.
3. Screenshot is downscaled and sent to the AI provider.
4. AI returns actions; Master dispatches each via `Execute`.
5. Loop repeats until AI signals done or `max_steps` is reached.
6. State is tracked in Redis under `instruct:{browser_id}`.

---

## 4. gRPC Service

```proto
service BrowserAgent {
  rpc Execute(BrowserCommand) returns (CommandResult);
}
```

`CommandResult.screenshot` is only populated for screenshot commands. All other commands return `success`/`error_message` only.

---

## 5. Configuration

* **`rustmani.yaml`** — Global settings: Redis, Flux, AI provider, resolution config, API keys.
* **`flux.yaml`** — Serverless triggers and resource limits for browser agent deployments.

---

## 6. Technical Requirements
* **Language:** Rust
* **Transport:** gRPC (Master → BrowserAgent), HTTP (External API)
* **Browser Support:** Chrome and Firefox
