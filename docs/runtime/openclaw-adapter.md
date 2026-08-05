# Adapter Design — openclaw-runtime

**Status:** Implemented
**Date:** 2026-08-05
**Scope:** The `ai-os-runtime-api` crate, the `runtime-manager` crate, and the `openclaw-runtime` adapter crate that translate AI-OS actions into OpenClaw tool calls without exposing OpenClaw internals.

---

## 1. Crate Layout

The user-approved layout is `runtime/ai-os-runtime-api/`, `runtime/runtime-manager/`, and `runtime/openclaw-runtime/`. Note: the top-level `runtime/` directory currently holds the existing `ai-os-runtime` crate (Runtime Platform, layer 3). The new crates are **Execution-layer** abstractions and are declared as separate workspace members so they do not disturb `ai-os-runtime`.

```
runtime/
├── ai-os-runtime/            (existing — Runtime Platform: scheduler, session, task)
├── ai-os-runtime-api/        (NEW — generic Runtime trait + types, this doc §3)
│   ├── Cargo.toml            package = "ai-os-runtime-api"
│   ├── src/
│   │   ├── lib.rs            #![forbid(unsafe_code)], #![warn(missing_docs)]
│   │   ├── error.rs          RuntimeError (thiserror)
│   │   ├── capability.rs     Capability, CapabilityId, SideEffect
│   │   ├── action.rs         Action, ActionResult, ActionStatus
│   │   ├── observation.rs    Observation, ObservationKind
│   │   ├── runtime.rs        Runtime trait, RuntimeEvent, RuntimeHealth
│   │   ├── default_runtime.rs DefaultRuntime
│   │   ├── dispatcher.rs     RuntimeDispatcher (Arc<dyn Runtime> owner)
│   │   ├── schema.rs         JSON-Schema subset validator (type/properties/required/items/enum)
│   │   └── time.rs           now_ms()
│   └── tests/integration.rs
├── runtime-manager/          (NEW — registers runtimes, merges capabilities, routes actions)
│   ├── Cargo.toml            package = "ai-os-runtime-manager"
│   └── src/manager.rs        RuntimeManager (RwLock registry + CapabilityId routing)
└── openclaw-runtime/         (NEW — OpenClaw adapter, this doc §4)
    ├── Cargo.toml            package = "ai-os-openclaw-runtime"
    ├── src/
    │   ├── lib.rs
    │   ├── error.rs
    │   ├── config.rs         OpenClawRuntimeConfig (TransportConfig::Stdio, allowlist, overrides)
    │   ├── openclaw_runtime.rs  struct OpenClawRuntime: Runtime impl
    │   ├── mcp/                minimal MCP client — protocol.rs, transport.rs, client.rs
    │   ├── capability_map.rs   OpenClaw tool → CapabilityId translation
    │   ├── action_map.rs       CapabilityId + Action.input → tool arguments
    │   ├── observation_map.rs  MCP observation → Observation
    │   ├── result_map.rs       tools/call result → ActionResult
    │   └── mock/               (feature = "testkit") MockMcpServer for tests
    ├── tests/integration.rs
    └── Cargo.toml
```

### Layering

- `ai-os-runtime-api` depends on: `serde`, `serde_json`, `tokio`, `async-trait`, `thiserror`, `uuid`, `chrono`. It must **not** depend on `ai-os-openclaw-runtime` or any vendor crate.
- `ai-os-openclaw-runtime` depends on: `ai-os-runtime-api`, `tokio`, `serde_json`. The MCP client is implemented in-tree (newline-delimited JSON-RPC 2.0, `PROTOCOL_VERSION = "2025-03-26"`) with **no third-party MCP/WS dependency** (avoids AGENTS.md §16 approval; stdio transport via spawned subprocess).
- AI-OS brain crates (`brain-coordinator`) depend only on `ai-os-runtime-api`.

---

## 2. Integration Boundary

```
AI-OS Brain                       openclaw-runtime                    OpenClaw
─────────────                     ──────────────────                   ────────
Planner ── PlannedAction
   │
   ▼
Capability Registry ── capability merge at init (runtime.capabilities())
   │
   ▼
RuntimeDispatcher.execute(Action)
   │                     │
   │        ┌────────────┴───────────────────┐
   │        │ OpenClawRuntime (implements Runtime)│
   │        │   - validates input vs schema   │
   │        │   - capability_map: id → tool   │
   │        │   - action_map: input → args    │
   │        │   - gateway_client.call(tool)   │
   │        │   - result_map: result → output │
   │        └────────────┬───────────────────┘
   │                     │ tools/call (MCP) or WS RPC
   │                     ▼
   │              Gateway
   │               ├── channel extension (telegram.send, …)
   │               ├── tool implementation (browser, web, …)
   │               └── plugin/connector
   ▼
ActionResult ──► Cognitive Loop ──► Communication Logic ──► Telegram reply
```

**Rule:** AI-OS never imports `ai-os-openclaw-runtime`. Only the wiring point (e.g. `main.rs` or the platform bootstrap) constructs `OpenClawRuntime` and registers it as `Arc<dyn Runtime>`. Updating OpenClaw touches only `openclaw-runtime`.

---

## 3. runtime-api (generic crate)

Full trait in `docs/runtime/runtime-api.md`. Core surface:

```rust
#[async_trait::async_trait]
pub trait Runtime: Debug + Send + Sync {
    fn id(&self) -> RuntimeId;
    async fn initialize(&self) -> Result<(), RuntimeError>;
    async fn capabilities(&self) -> Vec<Capability>;
    async fn capability(&self, id: &CapabilityId) -> Option<Capability>;
    async fn execute(&self, action: Action) -> Result<ActionResult, RuntimeError>;
    async fn observe(&self) -> Vec<Observation>;
    async fn subscribe(&self) -> Result<tokio::sync::mpsc::Receiver<RuntimeEvent>, RuntimeError>;
    async fn health(&self) -> RuntimeHealth;
}
```

### RuntimeDispatcher

A small helper in runtime-api that owns the `Arc<dyn Runtime>` and provides the brain-facing convenience:

```rust
pub struct RuntimeDispatcher {
    runtime: Arc<dyn Runtime>,
}

impl RuntimeDispatcher {
    pub fn new(runtime: Arc<dyn Runtime>) -> Self;

    /// Resolve capability → execute action, mapping any per-action failure
    /// into a typed ActionResult. Returns Err only for transport-level failures.
    pub async fn dispatch(&self, capability: CapabilityId, input: serde_json::Value) -> Result<ActionResult, RuntimeError>;

    /// Merge runtime capabilities into the brain's CapabilityRegistry.
    pub async fn merge_capabilities(&self, registry: &mut CapabilityRegistry) -> Vec<Capability>;
}
```

---

## 4. openclaw-runtime (adapter)

### 4.1 Config

```rust
pub struct OpenClawRuntimeConfig {
    pub runtime_id: String,          // "openclaw"
    pub gateway_endpoint: String,    // "ws://127.0.0.1:18789" or MCP stdio command
    pub auth_token: String,          // loaded from secrets, never logged
    pub transport: OpenClawTransport, // McpStdio | McpStreamableHttp | GatewayWs
    pub capability_overrides: HashMap<CapabilityId, String>, // id → openclaw tool name
}
```

### 4.2 Capability map (OpenClaw → AI-OS)

Translation table mapping OpenClaw tools/channel actions to AI-OS capability IDs. Kept in `capability_map.rs`; loaded from config so OpenClaw updates are config-only.

| AI-OS CapabilityId | OpenClaw implementation | Notes |
|---|---|---|
| `telegram.post_message` | telegram channel outbound `sendText` | via gateway tool `message` / channel tool |
| `telegram.send_media` | telegram channel outbound `sendMedia` | |
| `slack.post_message` | slack channel outbound | |
| `web.search` | `web-search` tool | |
| `web.fetch` | `web-fetch` tool | |
| `browser.navigate` | browser extension (CDP) | capability on demand |
| `fs.read` / `fs.write` | core `read`/`write` tools | optional — native-runtime may own fs |
| `email.send` | **GAP** — no OpenClaw email | see `email-send-integration.md`; implemented as mock MCP email plugin in this phase |

Capability id naming is owned by AI-OS (`ai-os-runtime-api` conventions), so adding or renaming an OpenClaw tool never changes brain code.

### 4.3 Action translation (AI-OS Action → OpenClaw tool call)

```
Action { capability: "telegram.post_message",
         input: { "chat_id": "-100123…", "text": "Hello" } }
   │ capability_map.resolve("telegram.post_message") → tool "message"
   │ action_map.build(tool, input) → args { "channel": "telegram",
   │                                        "target": "-100123…",
   │                                        "text": "Hello" }
   ▼
gateway_client.call_tool("message", args)      // MCP tools/call or WS RPC
```

### 4.4 Observation translation (OpenClaw inbound → AI-OS Observation)

Inbound channel events (classified by OpenClaw as `user_request`) are mapped:

```
OpenClaw inbound envelope → Observation {
  source: CapabilityId("telegram.receive"),   // connector that observed it
  kind: InboundMessage,
  channel_id: chat id,
  sender: user id,
  payload: { "text": "...", "chat_id": "...", "reply_to": ... },
}
   │
   ▼
Cognitive Loop (World Understanding) — AI-OS interprets, plans, decides
```

### 4.5 Result translation (OpenClaw result → ActionResult)

| OpenClaw result | ActionResult |
|---|---|
| success content | `Succeeded { output }` |
| tool error | `Failed { code: "tool_error", message, retryable }` |
| timeout/cancel | `TimedOut` / `Cancelled` |
| unknown capability | `Failed { code: "capability_not_found" }` |

### 4.6 Process model

- OpenClaw Gateway runs as a **supervised sidecar** (systemd unit under `services/`, or Runtime Supervisor task). AI-OS supervises it; if the Gateway dies, `Runtime::health()` reports `Unavailable` and the dispatcher rejects actions with a retryable error.
- The adapter connects at `initialize()` and verifies `health()` before merging capabilities.

---

## 5. Security

1. **Auth token** loaded from secrets (env/vault), `#[serde(skip_serializing)]`, never logged.
2. **Action input validated** against the capability's `input_schema` (JSON Schema) before forwarding — rejects malformed payloads at the AI-OS boundary.
3. **Capability allowlist** — only capabilities merged into the brain registry are callable; the Planner cannot address arbitrary OpenClaw tools.
4. **No shell interpolation** — all parameters are structured JSON, matching OpenClaw's tool contract (no string command building).
5. **Trace propagation** — `trace_id` flows AI-OS → adapter → OpenClaw, so actions are replayable.

---

## 6. Testing

| Test | Scope |
|---|---|
| Runtime trait unit tests | happy path, error path, default runtime, dispatcher merge |
| Capability map round-trip | id → tool → id stability |
| Action/result/observation maps | unit tests with fixture JSON |
| Adapter integration (mock gateway) | a fake MCP/WS server; verify translate+call+translate |
| Live smoke (optional, gated) | against a real OpenClaw gateway in a sandbox |

All tests deterministic; no sleeps/timeouts (mock transport).

---

## 7. Replaceability Proof (Acceptance)

To replace OpenClaw with a native runtime:
1. Implement `Runtime` in a new `native-runtime` crate (same trait).
2. Register `Arc::new(NativeRuntime)` at the wiring point instead of `OpenClawRuntime`.
3. Re-run `cargo test --workspace` — zero changes in `brain/` or `runtime-api`.

This is the core success criterion: **OpenClaw remains replaceable; AI-OS never depends on OpenClaw types.**
