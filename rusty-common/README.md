# rusty-common

Shared library used by `rusty-server` and `rusty-agent`.

## Contents

| Module | Description |
|---|---|
| `state` | Core types: `BrowserInfo`, `BrowserState`, `ContextInfo` |
| `redis_store` | Redis-backed registry for browsers, contexts, and instruct state |
| `config` | `RustyConfig` loaded from `rusty.yaml` |
| `flux` | `FluxClient` for spawning browser agents |
| `ai` | AI provider interface + OpenAI/OpenRouter implementations |
| `error` | Shared `RustyError` type |
