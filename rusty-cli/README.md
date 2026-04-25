# rusty-cli

A thin CLI for the rusty-server HTTP API.

## Install

```bash
cargo install --path rusty-cli
```

Or build from source:

```bash
cd rusty-cli
cargo build --release
# binary: target/release/rusty-cli
```

## Configuration

The CLI reads its server URL and API key from three sources, in order of precedence:

1. Command-line flags: `--url`, `--api-key`
2. Environment: `RUSTY_URL`, `RUSTY_API_KEY`
3. Stored config: `~/.rusty-cli.json` (Windows: `%USERPROFILE%\.rusty-cli.json`)

Save defaults so you don't have to pass them every time:

```bash
rusty-cli env set url http://127.0.0.1:8080
rusty-cli env set api-key <your-key>
rusty-cli env show
```

## Quickstart

```bash
# First-time setup on a fresh server: generate certs, register agent function
rusty-cli init

# Spawn a browser. The returned execution_id is remembered as the "last browser"
# so you can omit it from subsequent commands.
rusty-cli browser spawn

# Drive it
rusty-cli browser navigate https://example.com
rusty-cli browser screenshot > shot.json
rusty-cli browser instruct "find the login link and click it"
rusty-cli browser logs

# Clean up
rusty-cli browser close-all
rusty-cli teardown     # also terminates Flux nodes
```

## Commands

### Top-level

| Command | What it does |
|---|---|
| `init` | `POST /initialize/` — provision certs and register the agent function |
| `teardown` | `DELETE /teardown/` — close all browsers and terminate nodes |
| `env set <key> <value>` | Persist a config value (`url`, `api-key`) |
| `env show` | Print current config and file location |

### Browser lifecycle

| Command | What it does |
|---|---|
| `browser spawn [--identity <json>]` | Spawn an agent. Stores the returned `execution_id` as "last browser" |
| `browser list` | List active browsers |
| `browser get [id]` | Get one browser |
| `browser close [id]` | Close and deregister one browser |
| `browser close-all` | Close every browser |
| `browser create-context [id]` | Open a new tab |
| `browser close-context [id] <context_id>` | Close a tab |

### Navigation / interaction

| Command | Notes |
|---|---|
| `browser navigate [id] <url> [--wait-until <event>]` | Load a URL |
| `browser click [id] <x> <y>` | Pixel-coordinate click |
| `browser node-click [id] <node_id>` | Click by `node_id` (get one from `find-node`) |
| `browser type [id] <text> [--node-id <n>]` | Type into focused element or a specific node |
| `browser scroll-by [id] <y>` | Positive = down |
| `browser scroll-to [id] <node_id>` | Scroll a node into view |
| `browser send-keys [id] <keys>` | e.g. `"Backspace, Backspace"` |
| `browser hold-key [id] <key>` | e.g. `"Backspace3000"` = hold for 3000 ms |

### DOM queries

| Command | Notes |
|---|---|
| `browser find-node [id] <css-selector>` | Returns a `node_id` for use by click/type/fetch |
| `browser wait-for-node [id] <css-selector> [--timeout-ms <n>]` | Default timeout 5000 ms |
| `browser fetch-html [id] [--node-id <n>]` | Inner HTML of a node (or full doc if omitted) |
| `browser fetch-text [id] <node_id>` | Inner text |
| `browser ui-map [id]` | Dump the accessibility tree |
| `browser ui-map-diff [id]` | Diff against the last `ui-map` call |
| `browser eval [id] <script>` | Evaluate JavaScript and print the result |

### AI / logs

| Command | Notes |
|---|---|
| `browser instruct [id] <instruction>` | Runs async on the server (returns 202 immediately). Use `logs` to follow |
| `browser logs [id]` | Fetch Flux execution logs |

## ID resolution

Almost every `browser` subcommand takes an optional `[id]` positional. When omitted, the CLI uses the last `execution_id` returned from `spawn`, stored in `~/.rusty-cli.json`. To target a different browser without changing the stored default, pass the id explicitly.

## Notes

- All endpoints require the API key; the CLI sends it as `X-API-Key`.
- `instruct` is fire-and-forget from the CLI's perspective — the server runs the loop in the background and the command returns 202. Poll `logs` to follow progress.
- This crate is intentionally excluded from the workspace (`Cargo.toml`) to keep `reqwest`'s `blocking` feature out of the server build. Build it from its own directory.
