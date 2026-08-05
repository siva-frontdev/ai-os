# RFC-0007: External Runtime Integration (Runtime API)

| Field | Value |
|---|---|
| **Status** | Accepted |
| **Author** | AI-OS Architecture Team |
| **Phase** | 8 |
| **Created** | 2026-08-05 |
| **Updated** | 2026-08-05 |
| **Requires** | RFC-0005 (Execution Platform), RFC-0006 (Intelligence Platform) |
| **Supersedes** | None |

## Abstract

This RFC proposes a generic **Runtime API** — a trait-interface that separates AI-OS cognition from external execution substrates — and an **OpenClaw runtime adapter** as its first implementation. AI-OS retains sole ownership of understanding, memory, reasoning, planning, reflection, learning, and decision-making. OpenClaw (a Node.js personal-AI-assistant codebase) is treated strictly as an execution runtime: it exposes capabilities (channels, tools, connectors), executes actions, provides observations, and manages connectors/plugins — never as a competing brain. Future runtimes (Home Assistant, ROS, robot, native OSAL) implement the same trait without changing AI-OS.

## Motivation

AI-OS's Brain (Planner) currently plans only internal capabilities (`respond`, `search_memory`, `observe`, `schedule`). It has no generic way to drive the real world: send a Telegram message, search the web, control a browser, or email someone. Two failure modes exist if we wire such capabilities directly:

1. **Duplicate cognition.** Integrating an assistant product like OpenClaw "as an agent" would create a second planner, second memory, second reasoning loop, and second prompt stack — violating the core tenet that intelligence is a platform primitive owned by AI-OS.
2. **Vendor lock-in.** Importing OpenClaw types into the brain would couple AI-OS to one runtime. The execution substrate (messaging connectors, device runtimes) will evolve and must be replaceable.

The Runtime API addresses both: a stable, runtime-agnostic contract (`Runtime` trait) implemented by an adapter per runtime. AI-OS plans against capability IDs; the adapter translates to the runtime's tool calls. OpenClaw is analyzed and integrated as "hands and legs," its cognition subsystems bypassed entirely.

## Design

### Overview

```
AI-OS Brain (cognition: understand/remember/reason/plan/reflect/learn/decide)
        │  Action { capability: "email.send", input }
        ▼
Runtime API (ai-os-runtime-api)  ── Runtime trait (generic, vendor-free)
        │  implemented by
        ├── openclaw-runtime   (adapter → OpenClaw Gateway over MCP/WS)
        ├── native-runtime     (future: adapter → OSAL)
        ├── homeassistant-runtime (future)
        └── ros-runtime        (future)
```

Capabilities discovered via `Runtime::capabilities()` at init are merged into the brain's `CapabilityRegistry`, making the Planner aware of what is executable — declaratively, with zero planner code changes.

### Detailed Design

#### Interfaces

The full trait is specified in `docs/runtime/runtime-api.md`. Core surface:

```rust
pub struct CapabilityId(pub String);          // "email.send", "telegram.post_message"
pub struct Action { pub capability: CapabilityId, pub input: serde_json::Value, .. }
pub struct ActionResult { pub status: ActionStatus, pub output: Option<serde_json::Value>, .. }
pub struct Observation { pub source: CapabilityId, pub kind: ObservationKind, pub payload: serde_json::Value, .. }

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

OpenClaw-specific translation (capability map, action map, observation map, result map) lives only in `ai-os-openclaw-runtime`. Analysis and mapping decisions are in `docs/runtime/openclaw-analysis.md` and `docs/runtime/mapping.md`.

#### Events

| Direction | Event Type | Payload | Description |
|---|---|---|---|
| Published | `runtime.capabilities_changed` | `Vec<Capability>` | Runtime capabilities updated; brain registry re-merged |
| Published | `runtime.health_changed` | `RuntimeHealth` | Runtime ready/degraded/unavailable |
| Published | `runtime.observation` | `Observation` | Inbound message/device event for the Cognitive Loop |
| Published | `runtime.action_completed` | `ActionResult` | Action reached a terminal state |
| Consumed | `brain.decision.made` | `Decision` | Executor extracts Actions for the dispatcher |

#### Dependencies

| Dependency | Layer | Purpose |
|---|---|---|
| `serde` / `serde_json` | External | Structured payloads and JSON Schema validation |
| `tokio` | External | Async trait impls, channels |
| `async-trait`, `thiserror` | External | Async traits, error derivation |
| `ai-os-runtime` | Runtime | Optional: supervise the OpenClaw sidecar process |
| OpenClaw (sidecar) | External | Executes capabilities; NOT imported by AI-OS crates |

For the adapter's transport: a WS client (`tokio-tungstenite`) or an MCP client (`rmcp`/`fastmcp`) — dependency approval required per AGENTS.md §16 before implementation.

#### Configuration

| Key | Type | Default | Description |
|---|---|---|---|
| `runtime.enabled` | `bool` | `false` | Enable the external runtime layer |
| `runtime.transport` | `string` | `"mcp_stdio"` | `mcp_stdio` / `mcp_http` / `gateway_ws` |
| `runtime.endpoint` | `string` | `""` | Gateway WS URL or MCP endpoint/command |
| `runtime.auth_token` | `secret` | `""` | Loaded from secrets; never logged |
| `runtime.capability_allowlist` | `string[]` | `[]` | Which capabilities the Planner may address (empty = all discovered) |

#### Thread Model

- `RuntimeDispatcher` holds `Arc<dyn Runtime>`; `execute()` is awaited on the caller's async task.
- Observation push: adapter spawns a task reading the transport stream → `mpsc::Receiver<RuntimeEvent>` → consumed by the Cognitive Loop ingest task.
- Shared state: no locks beyond the transport client's internal buffers.

#### Lifecycle

| Phase | Action |
|---|---|
| **Init** | Read config → construct adapter → `runtime.initialize()` → merge `capabilities()` into brain registry |
| **Start** | Spawn observation-ingest task; publish `runtime.health_changed(Ready)` |
| **Stop** | Stop ingest task; drop adapter; publish `runtime.health_changed(Unavailable)` |

#### Error Handling

| Error | Recovery |
|---|---|
| Transport failure | Retry with backoff; health = `Degraded` |
| Gateway sidecar down | Supervisor restarts sidecar; health = `Unavailable`; actions rejected retryable |
| Unknown capability | `capability_not_found`; Planner cannot have planned it (registry-driven) |
| Action timeout | `ActionStatus::TimedOut`; deadline enforced by dispatcher |

### Security Considerations

- Auth tokens from secrets only; `#[serde(skip_serializing)]`; never logged.
- All action inputs validated against the capability's JSON Schema at the AI-OS boundary.
- Capability allowlist limits what the Planner may address; no arbitrary tool invocation.
- Structured JSON parameters only — no shell interpolation (matching OpenClaw's tool contract).
- `trace_id` propagates AI-OS → adapter → runtime for replayable audit.

### Performance Considerations

- Action dispatch adds one JSON round-trip through the adapter (~ms). Sub-ms capabilities should use the existing Execution Platform's direct path instead.
- Capability merge is a one-time init cost.
- Observation ingest is a single async task; bounded channel prevents backpressure blowup.

### Testing Strategy

- Unit tests: Runtime trait, DefaultRuntime, capability/action/observation/result maps, dispatcher merge.
- Integration (mock transport): fake MCP/WS server; assert translate → call → translate.
- Cross-crate: brain → dispatcher → adapter → mock gateway for the `email.send` flow.
- Live smoke (gated): real OpenClaw gateway in a sandbox.
- Benchmarks: adapter round-trip latency (Criterion).

## Drawbacks

1. **Node sidecar.** OpenClaw requires a Node.js runtime and a supervised process — added operational surface and a second runtime dependency.
2. **Translation overhead.** Every capability crosses a typed boundary; complex tool schemas add adapter maintenance.
3. **Schema drift.** OpenClaw tool schema changes must be absorbed in the adapter (mitigated by config-driven capability maps).

## Alternatives Considered

| Alternative | Reason for Rejection |
|---|---|
| Integrate OpenClaw as a second agent (its own loop/memory/prompts) | Creates duplicate planner, memory, reasoning, and prompts — violates the vision (AI-OS owns cognition). |
| Hard-code OpenClaw types into the brain | Vendor lock-in; future runtimes would require brain changes; violates replaceability. |
| Route everything through MCP directly from the brain | Still binds the brain to a specific protocol surface; the Runtime trait keeps the brain protocol-agnostic. |
| Wait for the existing Execution Platform (RFC-0005) to cover connectors | RFC-0005 covers subprocess/WASM/container execution, not external messaging/connector ecosystems. Complementary, not competing. |

## Open Questions

Resolved during implementation (2026-08-05):

1. **Location**: user-approved layout `runtime/ai-os-runtime-api`, `runtime/runtime-manager`, `runtime/openclaw-runtime` as new workspace members; the existing `ai-os-runtime` crate is untouched.
2. **Transport**: MCP (newline-delimited JSON-RPC 2.0 over stdio or in-memory duplex). Implemented as a minimal MCP client with zero new third-party dependencies.
3. **Email**: a mock MCP email plugin exposes `email.send` (recommended shape for a future OpenClaw email channel extension). AI-OS ships no SMTP logic.
4. **Observations**: `Runtime::observe()`/`subscribe()` ingest path via MCP `notifications/observation`; the brain is unchanged.

## Implementation Plan

Status of each step (2026-08-05):

1. ✅ `runtime/ai-os-runtime-api` crate: trait, types, DefaultRuntime, dispatcher, schema validation, unit tests.
2. ✅ `runtime/runtime-manager` crate: register / initialize / merge / capability-routed dispatch / observe / health, unit tests.
3. ✅ `runtime/openclaw-runtime` crate: config, capability/action/observation/result maps, MCP client + transports, unit tests.
4. ✅ `tests/runtime-integration` cross-crate tests: registration, dispatch, runtime swap, email flow (10 tests).
5. ⬜ Capability merge into the brain `CapabilityRegistry` at app wiring time (proven by `merge_capabilities` in tests; wiring into the app is future work).
6. ⬜ Live smoke against a sandboxed OpenClaw gateway (requires the real OpenClaw MCP server).

## Unresolved Topics

- Additional runtimes (native OSAL, Home Assistant, ROS, robot).
- Email inbound (IMAP) and richer communication capabilities.
- OAuth flows for channel/connector authentication managed by AI-OS.
- Remote/multi-node runtimes.
- App-level wiring: registering the runtime manager + OpenClaw runtime in the platform bootstrap, and feeding inbound observations into the Cognitive Loop.

## References

- [OpenClaw Analysis](../runtime/openclaw-analysis.md)
- [Runtime API Design](../runtime/runtime-api.md)
- [OpenClaw Mapping](../runtime/mapping.md)
- [Adapter Design](../runtime/openclaw-adapter.md)
- [email.send Integration](../runtime/email-send-integration.md)
- [RFC-0005: Execution Platform](RFC-0005-execution-platform.md)
- OpenClaw source (read-only): `~/openclaw`, commit `80da6166`
