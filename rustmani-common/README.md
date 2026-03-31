# rustmani-common

Shared library used by `rustmani-server` and `rustmani-agent`.

## Contents

| Module | Description |
|---|---|
| `state` | Core types: `BrowserInfo`, `BrowserState`, `ContextInfo` |
| `redis_store` | Redis-backed registry for browsers, contexts, and instruct state |
| `config` | `RustmaniConfig` loaded from `rustmani.yaml` |
| `flux` | `FluxClient` for spawning browser agents |
| `ai` | AI provider interface + OpenAI/OpenRouter implementations |
| `error` | Shared `RustmaniError` type |
