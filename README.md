<h1 align="center">
  <img src="https://rustybrowser.com/logos/rusty-browser_inv.png" height="50" valign="middle" />
  &nbsp;Rusty Browser
</h1>

**Distributed AI-driven browser automation at scale — built in Rust.**

Rusty is a serverless browser automation platform. You spawn browser agents on demand via an HTTP API, send them commands (navigate, click, type, screenshot, scroll, eval JS), and drive them with natural language through an AI instruct engine. Each agent runs in isolation, registers itself back to the master over gRPC, and is torn down when you're done.

---

[![Demo: Setup Rusty Browser on your pc in 6 minutes](https://img.youtube.com/vi/Y7O2Gfq2A-k.jpg)](https://youtu.be/Y7O2Gfq2A-k)

---

## Why Rusty over browser-use, Stagehand, or Playwright?

Most browser automation tools treat the browser as a local subprocess. That works for one browser. It doesn't work for fifty.

| | Rusty | browser-use | Stagehand | Playwright |
|---|---|---|---|---|
| Language | Rust | Python | TypeScript | JS/Python/Java |
| Architecture | Distributed (serverless agents) | Single-process | Cloud-managed | Single-process |
| Scale | Hundreds of concurrent agents | Limited by machine | Limited by plan | Limited by machine |
| Stealth | Built-in identity + proxy per agent | None | None | None |
| AI | Optional, per-agent | Core dependency | Core dependency | None |
| Infrastructure | Self-hosted with Flux | Local | Browserbase cloud | Local |
| Security | Mutual TLS, cert pinning | None | Managed | None |
| Overhead | Single Rust binary | Python runtime | Node.js runtime | Node.js runtime |

**Rusty is for when you need browsers to behave like serverless functions** — spawn on demand, run independently, scale horizontally, and clean up automatically. It is not a wrapper around Playwright or Puppeteer. It drives Chromium directly via the [WebDriver BiDi](https://w3c.github.io/webdriver-bidi/) protocol through [rustenium](https://github.com/dashn9/rustenium).

---

## Architecture

```
┌─────────────────────────────────────────────────┐
│                  Your Application                │
│                  HTTP REST API                   │
└───────────────────────┬─────────────────────────┘
                        │
                        ▼
           ┌────────────────────────┐
           │     rusty-server    │
           │  Redis · Flux · AI     │
           └──────┬─────────────────┘
      spawn via   │           gRPC/TLS commands
         Flux     │      ┌────────────────────────┐
                  └─────►│    rusty-agent       │
                         │  Chromium · Identity    │
                         │  Proxy · gRPC server    │
                         └────────────────────────┘
                         (one agent per browser instance)
```

**Lifecycle:**
1. `PUT /browsers/` — server spawns an agent via Flux, returns `execution_id`
2. Agent starts, launches Chromium, detects its public/private IP, registers back to master via gRPC
3. Server stores connection info in Redis, agent is now addressable
4. Send commands via REST — server forwards over gRPC to the agent
5. `DELETE /browsers/{id}/` — server sends `CloseBrowser`, agent exits, Redis entry is cleared
6. Stale agents that never register within the configured timeout are auto-cancelled

---

## Workspace

| Crate | Role |
|---|---|
| `rusty-server` | HTTP API + gRPC master + AI instruct engine |
| `rusty-agent` | Serverless browser agent (gRPC server over TLS) |
| `rusty-common` | Shared types, Redis store, Flux client, config, AI provider |
| `rusty-proto` | Protobuf definitions and generated Rust bindings |

---

## HTTP API

### Initialization

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/initialize/` | Generate TLS certs, register and deploy the agent function to Flux |

### Browsers

| Method | Path | Description |
|--------|------|-------------|
| `PUT` | `/browsers/` | Spawn a new browser agent |
| `GET` | `/browsers/` | List all active browsers |
| `DELETE` | `/browsers/` | Delete all browsers |
| `DELETE` | `/teardown/` | Delete all browsers and terminate all Flux nodes |
| `GET` | `/browsers/{id}/` | Get browser info |
| `DELETE` | `/browsers/{id}/` | Close and deregister a browser |

### Contexts

| Method | Path | Description |
|--------|------|-------------|
| `PUT` | `/browsers/{id}/contexts/` | Create a new browsing context |
| `DELETE` | `/browsers/{id}/contexts/{ctx_id}/` | Close a context |

### Commands

| Method | Path | Description |
|--------|------|-------------|
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

---

## Configuration

See [`rusty-server/example.rusty.yaml`](rusty-server/example.rusty.yaml) for a full annotated reference.

Start the server:

```sh
RUSTY_CONFIG=rusty.yaml cargo run --release --bin rusty
```

### Local development (no Flux)

Set `flux.local_binary` in your config to spawn agents as local subprocesses instead of deploying via Flux. Agent stdout/stderr is forwarded through the server's tracing output, prefixed with the execution ID.

```yaml
flux:
  local_binary: "cargo run -p rusty-agent --"  # or path to a built binary
```

Agent logs are emitted under the `rusty_agent` target and can be filtered independently:

```sh
RUST_LOG=rusty=info,rusty_agent=debug cargo run -p rusty-server
```

---

## Initialization

Before spawning any browsers, call `POST /initialize/` once. This generates TLS certs, registers the agent function with Flux, downloads the agent binary, bundles everything into a zip, and deploys it. Re-run after any agent code changes or when rotating TLS certs.

---

## Proxy Support

See [`rusty-server/example.agent-proxies.yaml`](rusty-server/example.agent-proxies.yaml). Proxies are geo-matched to the browser identity and bundled into each agent at initialization.

---

## TLS Security Model

- **Master cert** — generated once at server startup, stored in Redis, bundled into every agent. Agents use it to verify the master's identity (certificate pinning).
- **Agent cert** — generated at `/initialize/`, stored in Redis, bundled into every agent. The server fetches it from Redis to verify each agent it connects to.
- **Tunnel mode** — set `grpc_server_url` in config (e.g. an ngrok URL). The server automatically switches agents to native TLS verification instead of the pinned master cert.

---

## Setup

### 1. Install Flux

Rusty Browser uses [serverless-flux](https://github.com/dashn9/serverless-flux) to deploy browser agents on AWS and GCP at scale. Follow the setup instructions in that repo first, then come back with your Flux URL and API key.

### 2. Install Rusty Browser

Download the latest release binary for your platform from the [releases page](https://github.com/dashn9/rusty-browser/releases).

### 3. Configure

Copy the example config:

```sh
cp example.rusty.yaml rusty.yaml
```

Paste your Flux URL and API key:

```yaml
flux:
  base_url: "https://your-flux-url"
  api_key: "your_flux_api_key"
  function_name: "rusty-agent"
```

### 4. Run

```sh
RUSTY_CONFIG=rusty.yaml ./rusty
```

### 5. Initialize

Call this once before spawning any browsers — it generates TLS certs, bundles the agent, and deploys it to Flux:

```sh
curl -X POST http://localhost:8080/initialize/
```

Re-run after any agent code changes or when rotating certs.

---

## Building

Requires: Rust 1.80+, `protoc` on PATH.

```sh
cargo build --release
```

---

## License

MIT
