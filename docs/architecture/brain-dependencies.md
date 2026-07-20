# Brain Platform — Crate Dependency Matrix

Allowed and prohibited dependency relationships between Brain Platform crates (Layer 6) and lower layers.

---

## Layer Boundaries

Layer 6 (Brain) depends on:

| Layer | Crate(s) | Role |
|---|---|---|
| 2 (Core) | `ai_os_core` | EventBus, Service trait, Logger, lifecycle |
| 3 (Runtime) | `ai_os_runtime` | Task, Session, Permission, Scheduler |
| 4 (Memory — Phase 5) | `memory_core` | Types, Timestamp |
| 4–5 (Memory — Phase 5) | `memory_episodic`, `memory_semantic`, `memory_working`, `memory_context` | Knowledge retrieval and storage |

Layer 6 must **never** import from Layer 7 (Execution, future) or Layer 5 (Perception, future).

---

## Dependency Matrix

Columns: what the column crate **may** depend on.  
Rows: the crate being described.

| Crate | brain-core | brain-model | brain-goals | brain-planner | brain-reasoner | brain-decision | brain-policy | brain-reflection | brain-workflow | brain-learning | brain-coordinator | Core | Runtime | Memory |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| **brain-core** | — | | | | | | | | | | | ✓ | ✓ | ✓ |
| **brain-model** | ✓ | — | | | | | | | | | | | | |
| **brain-goals** | ✓ | opt | — | | | | | | | | | | | |
| **brain-planner** | ✓ | ✓ | | — | | | | | | | | | | |
| **brain-reasoner** | ✓ | ✓ | | | — | | | | | | | | | |
| **brain-decision** | ✓ | ✓ | | | | — | ✓ | | | | | | | |
| **brain-policy** | ✓ | | | | | | — | | | | | | | |
| **brain-reflection**| ✓ | | | | | | | — | | | | | | |
| **brain-workflow** | ✓ | opt | | | | | | | — | | | | | |
| **brain-learning** | ✓ | | | | | | | | | — | | | | ✓ |
| **brain-coordinator** | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | — | ✓ | ✓ |

**Key:** ✓ = allowed dependency; `opt` = optional dependency; blank = forbidden; `—` = self-reference not applicable

---

## Dependency Descriptions

### brain-core

The shared foundation crate. Must have zero dependencies on other Brain crates. It defines all Brain-internal ID types (newtypes around `Uuid`), shared context structs, the `CognitiveBudget` and `BudgetUsage` types, the `BrainState` FSM enum, the `BrainError` error hierarchy, and the canonical event structs.

**Public interface:** types and events only. No logic.

---

### brain-model

Provider-neutral model abstraction layer. Depends only on `brain-core` (for `CognitiveTimestamp`, `Confidence`, `GoalId`, etc.). It must not import any specific model provider SDK (OpenAI, Anthropic, Ollama, etc.). Implementations are loaded at runtime via the `ModelProviderRegistry`.

**Optional direction:** The Brain layer does not depend on any specific provider. Providers register themselves.

---

### brain-goals

Goal lifecycle management. Depends on `brain-core` for types and events. Optionally depends on `brain-model` if goal validation involves NLP classification (e.g., categorising a user-stated goal). Never depends on `brain-reasoner`, `brain-planner`, or `brain-decision` directly — uses events to trigger those phases.

---

### brain-planner

Goal decomposition and plan generation. Depends on `brain-core` and `brain-model` (to call a planning-capable model). Does **not** depend on `brain-reasoner` (generates its own hypotheses via the model) or `brain-decision` (policy is checked after the plan is produced).

**Tool planning is abstract:** `Planner` returns `ToolRequirement` specifications, not OS commands. A `ToolRequirement` declares a capability (`ToolCapabilityId`), input schema, output expectations, and side-effect declarations. The `ToolSelector` sub-trait (within `brain-planner`) matches requirements to `ToolCandidate` objects from a `ToolRegistry`. `ToolCandidate` providers are registered externally (Execution Platform on boot, operators via config).

---

### brain-reasoner

Reasoning engine. Depends on `brain-core` and `brain-model` (to call a reasoning-capable model). Never depends on `brain-planner` or `brain-decision`.

**Internal organisation:** Submodules (`reasoning/`, `inference/`, `constraints/`, `risk/`, `tradeoff/`, `hypothesis/`) each define their own trait with a `Default*` implementation. The top-level `Reasoner` trait delegates to submodules.

---

### brain-decision

Decision selection. Depends on `brain-core`, `brain-model` (optional, for model-based scoring), and `brain-policy` (mandatory — policy is consulted before every decision). Never depends on `brain-planner` or `brain-reasoner` directly.

**Contract:** `DecisionMaker::decide()` calls `PolicyEngine::evaluate()` first. If policy returns `passed == false` with a `blocking` violation, the decision is not made; a `DecisionRejected` event is emitted and the planner is asked to replan.

---

### brain-policy

**Standalone policy crate.** Depends only on `brain-core` for types. This isolation allows policy rules to be loaded, compiled, hot-reloaded, and evaluated without pulling in any Brain reasoning logic.

**Consumers:** `brain-decision` and `brain-coordinator`. `brain-decision` uses `PolicyEngine` to evaluate plans. `brain-coordinator` uses it for lifecycle guard rails (e.g., `{{pause_requested}}`).

---

### brain-reflection

Post-execution analysis. Depends on `brain-core` for `DecisionId`, `ReflectionId`, `OutcomeComparison`, and event types. Does not directly call `brain-learning`, `brain-memory`, or `brain-goals` — communicates via events.

---

### brain-workflow

Multi-step workflow orchestration with checkpoint/recovery. Depends on `brain-core` for workflow and checkpoint types. Optionally depends on `brain-model` for conditional branching decisions within a workflow step. Does not depend on `brain-coordinator` (workflows are self-driving once started).

**Composition primitives:** Sequential, Parallel, Conditional, FanOut, FanIn, TryCatch.

---

### brain-learning

Memory promotion and heuristic adjustment. Depends on `brain-core` for types and `memory_episodic` / `memory_semantic` (from Memory Platform, Phase 5) for promotion operations. Does not depend on `brain-reasoner`, `brain-planner`, or `brain-decision` directly — feeds adjustments back via events that those crates consume.

**No ML training.** Learning coordinates Memory operations only (promote episodic → semantic, adjust planner strategy weights, update reasoner heuristic weights).

---

### brain-coordinator

**Orchestrator.** Depends on **all** Brain crates plus Core, Runtime, and Memory. Owns the `CognitiveLoop`, the `BrainState` FSM, `CognitiveBudget` enforcement, and all event subscriptions.

**Owns:** `Coordinator` trait, `CognitiveLoop` trait, `CognitiveState` struct, BrainStart/Stop sequence.

---

## Communication Patterns

### Event-driven (no direct crate dependency)

| Source Crate | Target Crate | Mechanism |
|---|---|---|
| `brain-goals` | `brain-coordinator` | `brain.goal.created` |
| `brain-planner` | `brain-coordinator` | `brain.plan.created` |
| `brain-reasoner` | `brain-coordinator` | `brain.reasoning.finished` |
| `brain-decision` | `brain-coordinator` | `brain.decision.made` |
| `brain-reflection` | `brain-learning` | `brain.reflection.completed` |
| `brain-learning` | `brain-planner` | Planning adjustment events |
| `brain-learning` | `brain-reasoner` | Heuristic weight update events |

---

### Trait-based (compile-time interface)

| Calling Crate | Trait Defined In | Consumed From |
|---|---|---|
| `brain-coordinator` | `brain-core` | All brain crates via `&dyn Trait` object |
| `brain-decision` | `brain-policy` | `brain-policy` crate |
| `brain-planner` | `brain-model` | `brain-model` crate |

---

## Prohibited Dependency Patterns

| Pattern | Why Forbidden |
|---|---|
| `brain-reasoner` → `brain-planner` | Would create a cycle; reasoning should not directly mutate plans |
| `brain-planner` → `brain-reasoner` | Planner generates structure; reasoner evaluates it. They are sequential phases. |
| `brain-decision` → `brain-planner` | Decision happens after planning; no re-entrance |
| `brain-learning` → `brain-reasoner` directly | Learning feeds heuristics via Events, not direct calls |
| `brain-policy` → any crate except `brain-core` | Maintains isolation; policy must be independently reloadable |
| Any Brain crate → Layer 7+ (Execution, Perception, Intelligence) | Forward dependency violates layering |

---

## Future Extension Points

- **Phase 7 (Perception):** `brain-reasoner` will consume `perception.observation.new` events from Layer 5.
- **Phase 8 (Execution):** `brain-workflow` and `brain-decision` will dispatch `ToolCandidate` objects to an Execution Platform endpoint.
- **Phase 9 (Intelligence Integration):** ML-based classifiers may be introduced beneath `brain-reasoner`'s rule-based layer, still behind the `Reasoner` trait.

---

## Cross-References

- [`docs/architecture/brain.md`](brain.md) — crate overview and responsibilities
- [`docs/interfaces/brain-events.md`](brain-events.md) — canonical event catalog
- [`docs/cognitive-loop.md`](cognitive-loop.md) — cognitive cycle and state machines
- [`docs/rfc/RFC-0003-brain-platform.md`](../rfc/RFC-0003-brain-platform.md)
