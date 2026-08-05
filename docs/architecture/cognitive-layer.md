# AI-OS Cognitive Layer Architecture

## Overview

The Cognitive Layer is the autonomous intelligence engine of AI-OS. It transforms the platform from a passive execution framework into a proactive system that continuously observes, plans, executes, reflects, and learns.

This layer sits at the intersection of:
- **Runtime Platform** (Phase 3) — provides the `RuntimeManager`, MCP server, and plugin system
- **Brain Platform** (existing `brain/` crates) — provides planning, goals, reflection, learning
- **Memory Platform** (existing `crates/memory-*`) — provides persistent world model and semantic memory

The Cognitive Layer is implemented in `tests/cognitive-integration/` as the **integration glue** that wires these components together, enabling the full autonomous loop to run against the real MCP subprocess.

## Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────────┐
│                        COGNITIVE LAYER                              │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌──────────────┐    ┌──────────────┐    ┌────────────────────┐   │
│  │ Observation  │    │   Cognitive  │    │ Runtime-Aware      │   │
│  │ Layer        │───▶│   Loop       │───▶│ Executor           │   │
│  │ (sources)    │    │ (brain-      │    │ (dispatches real   │   │
│  │              │    │  coordinator)│    │  capabilities)     │   │
│  └──────────────┘    └──────────────┘    └─────────┬──────────┘   │
│         ▲                                           │             │
│         │                                           ▼             │
│  ┌──────────────┐                          ┌────────────────────┐ │
│  │ World Model  │◀─────────────────────────│ Runtime Manager    │ │
│  │ (memory-core)│     Observations         │ (MCP subprocess)   │ │
│  └──────────────┘                          └────────────────────┘ │
│         │                                           ▲             │
│         ▼                                           │             │
│  ┌──────────────┐    ┌──────────────┐    ┌─────────┴──────────┐  │
│  │ Goal Manager │    │   Planner    │    │  Reflection       │  │
│  │ (brain-goals)│    │ (brain-      │    │  Engine           │  │
│  └──────────────┘    │  planner)    │    │  (brain-reflection)│  │
│                      └──────────────┘    └────────────────────┘  │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

## Data Flow

### 1. Observation Phase
- **RuntimeObservationSource** polls `RuntimeManager.observe()` 
- Converts raw `Observation` events into text summaries
- Feeds into the `CognitiveLoopService` via the observation channel

### 2. Cognitive Cycle (brain-coordinator::CognitiveLoopService)
```
User Input / Observation
        │
        ▼
World Understanding (LLM interprets text → StructuredWorldUpdate)
        │
        ▼
Memory Evaluator (decides what knowledge to store)
        │
        ▼
Evolution Engine (stores selected knowledge in World Model)
        │
        ▼
Planner (produces Plan with PlannedAction list)
        │
        ▼
RuntimeAwareExecutor (dispatches runtime actions via RuntimeManager)
        │
        ▼
Decision (Communicate / Wait / UpdateMemory / Execute)
```

### 3. Execution Phase
- `RuntimeAwareExecutor.execute_plan()` filters plan actions:
  - In-memory actions (`respond`, `search_memory`, etc.) → handled by brain's `ActionExecutor`
  - Runtime capabilities (`email.send`, `filesystem.write`, etc.) → dispatched via `RuntimeManager.dispatch()`
- Real MCP subprocess executes the capability
- Result returned as `ActionResult`

### 4. Observation Feedback
- MCP server emits `status_changed` / `inbound_message` observations
- Next poll cycle, `RuntimeObservationSource` detects them
- Cycle repeats

### 5. Reflection (brain-reflection)
- After goal completion: `Reflector.reflect()` analyzes outcomes
- `LearningEngine.learn_from_lesson()` extracts patterns
- Patterns stored in semantic memory (`crates/memory-semantic`)

## Key Components

### CognitiveRuntimeBridge (`tests/cognitive-integration/src/bridge.rs`)
Assembles the full pipeline:
```rust
let config = OpenClawRuntimeConfig { ... };
let bridge = CognitiveRuntimeBridge::with_mcp_runtime(config).await;
bridge.initialize().await?;  // boots MCP, discovers capabilities
let manager = bridge.manager();
let executor = bridge.executor();
let obs_source = bridge.observation_source();
```

### RuntimeAwareExecutor (`tests/cognitive-integration/src/executor.rs`)
Dispatches plan actions targeting runtime capabilities:
- Maps planner `topics` format to capability-specific input schemas
- Uses `RuntimeManager.dispatch()` for real subprocess execution
- Returns `(capability, ActionResult)` pairs

### RuntimeObservationSource (`tests/cognitive-integration/src/source.rs`)
Wraps `RuntimeManager` as an `ObservationSource`:
```rust
let source = RuntimeObservationSource::new(manager);
while let Some(text) = source.poll().await {
    // feed to cognitive loop
}
```

## Capability Input Schema Mapping

The planner produces `PlannedAction { action_type, topics: Vec<String>, reason }`.
Runtime capabilities expect structured input. The bridge maps known capabilities:

| Capability | Planner Topics | Runtime Input |
|---|---|---|
| `email.send` | `[to, subject, body]` | `{"to": "...", "subject": "...", "body": "..."}` |
| `telegram.send` | `[chat_id, text]` | `{"chat_id": "...", "text": "..."}` |
| `telegram.inject_inbound` | `[chat_id, sender, text]` | `{"chat_id": "...", "sender": "...", "text": "..."}` |
| `filesystem.write` | `[path, content]` | `{"path": "...", "content": "..."}` |
| `filesystem.read` | `[path]` | `{"path": "..."}` |
| `calendar.create_event` | `[date, title, description]` | `{"date": "...", "title": "...", "description": "..."}` |
| `calendar.read` | `[date]` | `{"date": "..."}` |
| `github.create_issue` | `[title, body]` | `{"title": "...", "body": "..."}` |
| `github.read_repository` | `[owner, repo]` | `{"owner": "...", "repo": "..."}` |

Unknown capabilities fall back to `{"topics": [...], "reason": "..."}`.

## Testing

All integration tests in `tests/cognitive-integration/tests/pipeline.rs` run against the **real** `ai-os-mcp-server` binary:

| Test | Description |
|---|---|
| `cognitive_bridge_initializes_with_real_mcp` | Bridge boots MCP, discovers ≥10 capabilities |
| `cognitive_executor_dispatches_real_email` | `email.send` executes through real subprocess |
| `cognitive_executor_produces_observable_side_effect` | `telegram.inject_inbound` produces observation |
| `cognitive_mixed_plan_dispatches_runtime_only` | Mixed in-memory + runtime plan filters correctly |
| `observability_source_detects_runtime_events` | Observation source polls runtime events |
| `cognitive_executor_handles_unknown_capability` | Unknown capability returns typed failure |
| `full_cognitive_pipeline_observe_plan_execute_observe` | **Canonical test**: observe → plan → execute → observe result |

No mocks. Every test spawns the real subprocess.

## Configuration

The bridge is configured via `OpenClawRuntimeConfig`:
```rust
OpenClawRuntimeConfig {
    runtime_id: "cognitive-test".into(),
    transport: TransportConfig::Stdio {
        command: "ai-os-mcp-server".into(),
        args: vec![
            "--plugins".into(),
            "email,filesystem,github,calendar,telegram".into(),
            "--root".into(),
            "/tmp/test-root".into(),
        ],
    },
    ..Default::default()
}
```

Plugins are selected via `--plugins`. The `--root` constrains filesystem access.

## Security

- **Capability Policy** (`crates/capability-policy`) evaluates risk before dispatch
- **Confirmation Required**: `email.send`, `github.create_issue`, `calendar.create_event` require user confirmation
- **Denied by Default**: `filesystem.write_root` permanently denied
- **Sandbox**: Filesystem plugin rejects `..` path traversal
- **Audit**: Every dispatch decision logged

## Integration with Existing Layers

| Layer | Integration Point |
|---|---|
| Runtime (Phase 3) | `RuntimeManager`, `OpenClawRuntime`, `ai-os-mcp-server` |
| Brain | `CognitiveLoopService`, `Planner`, `ActionExecutor`, `GoalManager` |
| Memory | `InMemoryWorldModelStore`, `WorldUnderstandingService`, `EvolutionEngine` |
| Intelligence | `IntelligenceCoordinator` (LLM requests) |
| Capability Policy | `CapabilityGate::evaluate()` before dispatch (future) |

## Future Work

1. **Wire `CapabilityGate` into `RuntimeAwareExecutor`** — evaluate risk/confirmation before every dispatch
2. **Continuous Loop** — replace single `tick()` with event-driven `EventDriver` loop
3. **Long-term Memory Integration** — store lessons from `Reflector` into `memory-semantic::KnowledgeBase`
4. **Goal Manager Integration** — `GoalManager` creates goals → `cognitive_tick()` processes them
5. **Persistence** — World Model survives restart via `CompanionHost::save()/load()`

## References

- [World Model](./world-model.md)
- [Planner](./planner.md)
- [Executor](./executor.md)
- [Reflection](./reflection.md)
- [Event Bus](./event-bus.md)