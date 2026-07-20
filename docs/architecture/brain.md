# Brain Platform Architecture

## Purpose

The Brain Platform (Phase 6) is the cognitive orchestration layer of AI-native OS. It is responsible for reasoning, planning, decision-making, reflection, and coordinating intelligent behaviour. The Brain is **not** an LLM. LLMs are external reasoning engines consumed through the `brain-model` abstraction layer. The Brain must function even if different AI models are swapped in the future.

The Brain Platform depends on [Memory Platform](memory.md) (knowledge retrieval and storage), [Runtime Platform](runtime.md) (task execution and scheduling), and [Core Platform](core.md) (EventBus, Service, Logger). It does not depend on Perception or Execution.

---

## Refinements (Post-Approval)

Since initial approval, five architectural refinements have been incorporated:

1. **`brain-policy` crate extracted**: Policy evaluation, guard rails, rule compilation, and rule loading are moved out of `brain-decision` into a dedicated `brain-policy` crate. `brain-decision` consumes `brain-policy` traits; it does not own policy logic.
2. **`brain-reasoner` internal reorganization**: Reasoner's internals are split into submodules (`reasoning/`, `inference/`, `constraints/`, `risk/`, `tradeoff/`, `hypothesis/`). No new crates created.
3. **`CognitiveBudget`**: Every reasoning cycle receives a budget with limits on time, iterations, depth, branches, model calls, tokens, cost, and deadline. Planner, Reasoner, DecisionMaker, and Coordinator all consume it.
4. **Tool Planning** (abstract tools): The Planner returns `ToolRequirement` objects rather than OS-level commands. Execution Platform (Phase 8) will map `ToolRequirement` → `ToolCandidate` → actual dispatch.
5. **`BrainState` finite state machine**: Coordinator owns the current `BrainState`. States: Sleeping, Idle, Planning, Reasoning, WaitingModel, WaitingExecution, Reflecting, Learning, Recovering, Paused, Stopped.

See [cognitive-loop.md](cognitive-loop.md) for the full state machine, goal lifecycle, planner flow, reasoning flow, decision flow, reflection flow, learning flow, retry strategy, and PlantUML diagrams.

---

## Responsibilities

- **Reasoning**: Evaluate observations, context, and memory to generate structured reasoning traces. Generate and evaluate hypotheses. Detect contradictions and assess risk and tradeoffs. (Phase 9 adds ML-based classification as a fallback beneath rule-based reasoning.)
- **Planning**: Decompose goals into executable plans using abstract `ToolRequirement` specifications (not OS commands). Generate task graphs, resolve dependencies, identify parallelizable steps, optimise for cost/duration, and produce recovery plans.
- **Policies**: Separate policy layer (`brain-policy`) handling guard rails, rule evaluation, rule compilation, and policy loading. PolicyEngine evaluates plans before decisions are made.
- **Decision**: Select actions from evaluated options using configurable strategies. Enforce policy constraints, resolve conflicts, assign calibrated confidence scores, and produce explanations.
- **Goal Management**: Maintain a goal DAG (not tree — goals can have multiple parents). Track goal state. Support goal dependencies, prerequisites, and conflicts.
- **Reflection**: Post-execution analysis comparing expected vs actual outcomes, classifying mistakes, generating improvements, and producing lessons.
- **Learning**: Coordinate with Memory to identify knowledge worth promoting, consolidate episodic memory into semantic facts, and feed improvements back to the Planner and Reasoner. **No ML training.**
- **Workflow**: Long-running multi-step orchestration with checkpoint/recovery. Supports sequential, parallel, conditional, fan-out/fan-in, and try-catch composition patterns.
- **Coordination**: The Coordinator runs the primary cognitive loop, manages `BrainState`, enforces `CognitiveBudget`, and publishes/subscribes to all Brain and cross-layer events.
- **Model Abstraction**: Provider-neutral AI model interface. The Brain depends on `brain-model` traits; no Brain crate imports OpenAI, Anthropic, Ollama, or any specific model SDK.

---

## Crate Structure

```
brain/
├── Cargo.toml
├── brain-core/
├── brain-goals/
├── brain-planner/
├── brain-reasoner/
│   ├── src/reasoning/
│   ├── src/inference/
│   ├── src/constraints/
│   ├── src/risk/
│   ├── src/tradeoff/
│   └── src/hypothesis/
├── brain-decision/
├── brain-policy/ ← NEW: PolicyStore, PolicyEngine, PolicyEvaluator, PolicyCompiler, PolicyLoader, RuleSet, GuardRails
├── brain-reflection/
├── brain-workflow/
├── brain-learning/
├── brain-coordinator/
└── brain-model/
```

---

## Dependency Graph

```
┌──────────────┐
│ brain-core │ (depends on Core, Runtime, Memory)
└──────┬───────┘
       │
┌────────────────┼────────────────┐
       │ │ │
┌──────────────┐ ┌──────────────┐ ┌──────────────┐
│ brain-model │ │ brain-goals │ │brain-reasoner│
└──────┬───────┘ └──────┬───────┘ └──────┬───────┘
       │ │ │
       │ ┌───────────┼───────┐ │
       │ │ │ │ │ │
┌─────┴──┐ ┌─┴──┐ ┌────┴──┐┌───┴────┐┌──┴────┐
│planner │ │ref-│ │decision││workflow││learning│
└───┬────┘ │lec-│ └───┬────┘└───┬────┘└───┬───┘
     │ │tion │ │ │ │
     └──────┼─────┼────┘ │ │
           │ │ │ │
┌────┴─────┴───────────────┴─────────┐
│ brain-coordinator │
│ (depends on ALL brain crates) │
└─────────────────────────────────────┘

brain-policy (depends on brain-core only)
consumed by: brain-decision, brain-coordinator
```

### Dependency Rules Within Brain (Layer 6)

| Crate | May depend on | May NOT depend on |
|---|---|---|
| `brain-core` | Core (2), Runtime (3), Memory (4, 5) | Any other brain crate |
| `brain-model` | `brain-core` only | Any provider SDK, any other brain crate |
| `brain-goals` | `brain-core`, `brain-model` (optional) | `brain-policy`, `brain-coordinator` |
| `brain-planner` | `brain-core`, `brain-model` | `brain-policy`, `brain-reasoner` |
| `brain-reasoner`| `brain-core`, `brain-model` | `brain-planner`, `brain-decision` |
| `brain-decision`| `brain-core`, `brain-model`, `brain-policy` | `brain-reasoner`, `brain-planner` |
| `brain-policy` | `brain-core` only | All other brain crates |
| `brain-reflection`| `brain-core` | All other brain crates (except via events) |
| `brain-workflow` | `brain-core`, `brain-model` (optional) | `brain-coordinator` |
| `brain-learning` | `brain-core`, Memory (4, 5) | `brain-coordinator` |
| `brain-coordinator`| ALL brain crates + Core, Runtime, Memory | Nothing (top of Brain layer) |

---

## brain-core — Shared Foundation

### Responsibility

Single source of truth for all Brain-internal types. No Brain crate may define its own ID type for entities that cross crate boundaries — all use `brain-core` types.

### Inputs

- Type definitions from Core (`CognitiveTimestamp` from Memory, `TraceId`, `SessionId`)
- Trait definitions that all other Brain crates must implement (e.g., event types carry `event_type()`, `trace_id()`)

### Outputs

- Re-exports all ID types, context structs, budget types, and event structs
- Event type constants (string identifiers used for EventBus subscriptions)

### Internal Folder Layout

```
brain-core/src/
├── lib.rs               # crate docs, public re-exports
├── error.rs             # BrainError enum with thiserror
├── ids/
│   ├── mod.rs           # pub use all ID types below
│   ├── goal_id.rs       # GoalId(pub Uuid)
│   ├── plan_id.rs
│   ├── decision_id.rs
│   ├── thought_id.rs
│   ├── reflection_id.rs
│   ├── workflow_id.rs
│   ├── tool_id.rs
│   ├── tool_capability_id.rs
│   ├── checkpoint_id.rs
│   └── lesson_id.rs
├── types/
│   ├── mod.rs
│   ├── confidence.rs    # Confidence(pub f32) newtype
│   ├── budget.rs        # CognitiveBudget, BudgetUsage, BudgetDimension
│   ├── state.rs         # BrainState enum (FSM)
│   └── priority.rs      # GoalPriority, PlanPriority enums
├── context/
│   ├── mod.rs
│   ├── reasoning.rs     # ReasoningContext
│   ├── decision.rs      # DecisionContext
│   ├── planning.rs      # PlanningContext
│   └── goal.rs          # GoalContext
├── budget/
│   └── mod.rs           # BudgetChecker trait (exhausted, remaining_tokens, past_deadline, degraded)
├── state/
│   └── mod.rs           # BrainStateMachine: validate_transition(current, new) -> Result<()>
├── events/
│   ├── mod.rs           # Re-export all event structs
│   ├── goal.rs          # GoalCreated, GoalActivated, GoalCompleted, GoalCancelled, GoalFailed, GoalPaused
│   ├── plan.rs          # PlanCreated, PlanUpdated, PlanRejected
│   ├── reasoning.rs     # ReasoningStarted, ReasoningFinished, BudgetExhausted
│   ├── decision.rs      # DecisionMade, DecisionRejected, EscalationRequired
│   ├── reflection.rs    # ReflectionCompleted, OutcomeObserved
│   ├── learning.rs      # LearningCycleStarted, LearningCycleCompleted, PromotionApplied
│   ├── workflow.rs      # WorkflowStarted, WorkflowStepStarted, WorkflowStepCompleted, WorkflowCheckpointed
│   ├── coordinator.rs   # BrainStarted, BrainPaused, BrainResumed, BrainStopping, BrainStopped, BrainFailed
│   └── policy.rs        # PolicyEvaluated, PolicyReloaded
├── tool/
│   ├── mod.rs
│   ├── capability.rs    # ToolCapability, ToolParameter, ToolParameterType
│   ├── requirement.rs   # ToolRequirement
│   ├── candidate.rs     # ToolCandidate, SideEffect enum
│   └── registry.rs      # ToolRegistry trait
└── traits/
    └── mod.rs           # Shared trait bounds (BrainResult<T> type alias)
```

### Constraints

- `brain-core` must compile independently (no Brain crate dependencies).
- All ID types are newtype wrappers around `Uuid`; implement `From<Uuid>`, `Into<Uuid>`, `PartialEq`.
- All context structs carry a `trace_id: TraceId` field for distributed tracing.
- `BrainState` is an enum with no data fields (unit variants only). Transitions are validated by `state::BrainStateMachine`.

---

## brain-goals — Goal Lifecycle

### Responsibility

Maintain a goal state machine (DAG of goals with typed dependencies and priorities). Validate transitions. Emit lifecycle events. Detect cycles on activation.

### Inputs

- `GoalCreated` event (from operator, API, or workflow)
- `Memory` events (`memory.item.retrieved`, `memory.consolidation.completed`) to enrich context
- `CognitiveBudget` (optional per-goal override from the incoming event)

### Outputs

- `GoalActivated`, `GoalCompleted`, `GoalFailed`, `GoalCancelled`, `GoalPaused` events
- `GoalDependencyViolation` event when a dependency is unsatisfied
- `RetrieveGoal`, `RetrieveGoalStatus` responses to Coordinator queries

### Lifecycle

States: Pending → Active → Planning → Executing → Evaluating → Completed
  also: Evaluating → Recovering → Executing, Evaluating → Failed
  also: any → Paused (cooperative pause), any → Cancelled (operator cancel)

Transition to Active requires: all `blocking` dependencies resolved, no cycle in ancestor chain, goal store write lock obtained.

### Key Trait: GoalManager

| Method | Input | Output | Side Effects |
|---|---|---|---|
| `create(goal)` | `Goal` (name, description, type, priority, dependencies, deadline) | `GoalId` | Emits `GoalCreated`; writes to store |
| `get(goal_id)` | `GoalId` | `Option<Goal>` | None |
| `activate(goal_id)` | `GoalId` | `()` | Validates deps, emits `GoalActivated` |
| `complete(goal_id, outcome)` | `GoalId`, outcome text | `()` | Emits `GoalCompleted` |
| `fail(goal_id, error)` | `GoalId`, `BrainError` | `()` | Emits `GoalFailed` |
| `cancel(goal_id, reason)` | `GoalId`, reason text | `()` | Emits `GoalCancelled` |
| `status(goal_id)` | `GoalId` | `GoalStatus` | None |

---

## brain-planner — Planning with Abstract ToolRequirements

### Responsibility

Decompose a goal into an executable plan. Use abstract `ToolRequirement` specifications — never OS commands or service calls. Select the best plan alternative within the `CognitiveBudget`.

### Inputs

- Goal (from Coordinator, via `CognitiveLoop::tick`)
- `CognitiveBudget` (limits branches, iterations, token budget)
- `ReasoningContext` from ContextManager (observations, hypotheses)
- ToolRegistry (query for `ToolCandidate` matching each `ToolCapabilityId`)

### Outputs

- `ExecutablePlan` containing:
  - `Vec<PlanStep>` each with `Vec<ToolRequirement>`
  - `tool_requirements: Vec<ToolRequirement>` (flat list for policy evaluation)
  - `fallback_requirements` (alternative tool sets for retry)
  - `estimated_duration_ms`, `estimated_cost_cents`, `estimated_confidence`
  - `status: PlanStatus` (Draft / Approved / Rejected)
- Events: `PlanCreated`, `PlanUpdated`, `PlanRejected`

### ToolRequirement (Abstract Contract)

A `ToolRequirement` declares **what capability is needed**, not **which tool executes it**. It carries:

| Field | Purpose |
|---|---|
| `capability: ToolCapabilityId` | What the tool must be able to do (e.g., `"read_file"`, `"run_command"`, `"search_web"`) |
| `inputs: HashMap<String, ToolValue>` | Abstract input values (file path as string, not OS handle) |
| `expected_outputs: Vec<String>` | Named outputs the planner expects back |
| `timeout_ms: Option<u64>` | Soft timeout hint |
| `retry_policy: RetryPolicy` | How many retries on transient failure |
| `priority: u8` | Higher = must-have; lower = nice-to-have |

A `ToolCandidate` satisfies a `ToolRequirement`:

| Field | Purpose |
|---|---|
| `provider: String` | `"execution"`, `"perception"`, `"custom:foo"` |
| `capability: ToolCapabilityId` | Matches requirement |
| `estimated_cost: f64` | Cost estimate (USD) |
| `estimated_duration_ms: u64` | Latency estimate |
| `confidence: Confidence` | How well this tool matches |
| `side_effects: Vec<SideEffect>` | Declared side effects (FilesystemWrite, NetworkRequest, …) |

### Planner Flow (Contract)

1. `Planner::decompose(goal, budget)` is called.
2. Planner acquires a brief write lock on the goal, calls model: `generate_task_graph(goal, context)`.
3. `TaskGraphBuilder::resolve_dependencies(graph)` — topological sort; cycles return error.
4. Select planning strategy by budget priority: MEA (≥ 3), ForwardChaining (≥ 2), BackwardChaining (≥ 1), TemplateMatch (else).
5. For each strategy (up to `budget.max_branches`): generate `AlternativePlan`, estimate cost/duration/confidence.
6. `Planner::select_best(alternatives, criteria)` — minimise cost, maximise confidence.
7. Convert plan actions to `Vec<ToolRequirement>`.
8. `PlanOptimizer::optimize` if `budget.max_iterations > 0`: prune, merge, reorder, revalidate budget.
9. Return `ExecutablePlan` to Coordinator.

---

## brain-reasoner — Reorganized Internally

### Responsibility

Generate structured reasoning from observations, context, and memory. Produce and evaluate hypotheses. Detect contradictions. Assess risk and tradeoffs.

### Inputs

- `ReasoningContext` (goal_id, observations, hypotheses in progress, constraints, budget)
- Model provider (via `brain-model` `ReasoningModel` trait)
- Memory retrieval results (from Memory Platform) for grounding

### Outputs

- `ReasoningResult` containing the best hypothesis, confidence, reasoning trace
- Events: `ReasoningStarted`, `ReasoningFinished`, `ReasoningHypothesisGenerated`, `BudgetExhausted`

### Internal Submodules

Each submodule defines its own trait with a `Default*` implementation. The top-level `Reasoner` trait delegates:

| Submodule | Trait | Responsibility |
|---|---|---|
| `reasoning/` | `ReasoningEngine` | Primary entry point; orchestration |
| `inference/` | `InferenceEngine` | Chain-of-thought generation, inference |
| `constraints/` | `ConstraintSolver` | CSP-style constraint checking |
| `risk/` | `RiskAnalyzer` | Probabilistic risk assessment |
| `tradeoff/` | `TradeOffAnalyzer` | Pareto frontier, multi-criteria ranking |
| `hypothesis/` | `HypothesisGenerator` | Hypothesis generation and evaluation |

### Key Trait: Reasoner

| Method | Input | Output | Contract |
|---|---|---|---|
| `reason(ctx, budget)` | `ReasoningContext`, `CognitiveBudget` | `ReasoningResult` | Runs full reasoning cycle within budget |
| `generate_hypotheses(observations)` | `Vec<Observation>` | `Vec<Hypothesis>` | Produces initial candidate hypotheses |
| `evaluate(hypothesis, evidence)` | `Hypothesis`, evidence | `Confidence` | Updates confidence given new evidence |
| `select_best_hypothesis(hypotheses)` | `Vec<Hypothesis>` | `Option<Hypothesis>` | Returns highest-confidence candidate or `None` if all below threshold |

### Constraints

- Never held a lock across a model call.
- Reasoning depth is bounded by `CognitiveBudget::max_reasoning_depth`.
- If budget is exhausted mid-cycle: emit `BudgetExhausted`, return best hypothesis found so far (may be low-confidence).

---

## brain-decision — Policy-Conscious Decision

### Responsibility

Select actions from evaluated options. Enforce policy constraints. Resolve conflicts. Assign calibrated confidence scores. Produce human-readable explanations.

### Inputs

- `DecisionContext` (decision_id, plan_id, candidate options, policies, optional conflict, budget)
- `PolicyEvaluation` result (from `brain-policy::PolicyEngine`, called **before** selecting)
- `CognitiveBudget`

### Outputs

- `Decision` struct (selected option, alternatives considered, rationale, confidence, timestamp, explanation)
- Events: `DecisionMade`, `DecisionRejected`, `EscalationRequired`

### Key Trait: DecisionMaker

| Method | Input | Output | Contract |
|---|---|---|---|
| `decide(ctx, budget)` | `DecisionContext`, `CognitiveBudget` | `Decision` | Calls `PolicyEngine::evaluate` first. Blocks if policy rejects. |
| `select_action(options, criteria)` | `Vec<DecisionOption>`, `SelectionCriteria` | `DecisionOption` | Rank: confidence desc, cost asc, risk asc |
| `rank_options(options)` | `Vec<DecisionOption>` | `Vec<(DecisionOption, f64)>` | Returns sorted list with scores |

### Decision Flow (Contract)

1. Call `PolicyEngine::evaluate(ctx, plan)`. If `passed == false` with a `blocking` violation: emit `DecisionRejected`, return error. Do not call `select_action`.
2. Aggregate confidence evidence from model, context, and history → `ConfidenceScorer`.
3. Rank options by confidence (primary), cost (secondary), risk (tertiary).
4. `ConflictResolver::detect(options)`. On conflict: resolve or emit `EscalationRequired` if critical.
5. `select_action()` → `Decision`.
6. `ExplanationGenerator::explain(decision)` → stored in `Decision.explanation`.
7. Emit `DecisionMade`.

### Key Trait: ExplanationGenerator

| Method | Input | Output |
|---|---|---|
| `explain(decision)` | `Decision` | `DecisionExplanation` (rationale, candidates, evidence) |
| `format_for_operator(explanation)` | `DecisionExplanation` | `String` (formatted for human review) |

---

## brain-policy — NEW CRATE

### Responsibility

All policy logic lives here. Completely isolated from reasoning and planning. Owns rule loading, compilation, hot-reload, guard-rail enforcement, and policy evaluation.

### Inputs

- `PolicyConfig` (rulesets, guard rails, evaluation order) from config file or hot-reload
- `DecisionContext` and `ExecutablePlan` from `brain-decision`
- `CognitiveBudget` from Coordinator

### Outputs

- `PolicyEvaluation` (passed, violated_rules, warnings, allowance)
- Events: `PolicyEvaluated`, `PolicyReloaded`

### Key Traits

#### PolicyStore

| Method | Input | Output |
|---|---|---|
| `load_ruleset(name)` | ruleset name | `RuleSet` |
| `store_ruleset(ruleset)` | `RuleSet` | `()` |
| `list_rulesets()` | — | `Vec<String>` |
| `load_guard_rails()` | — | `Vec<GuardRail>` |
| `reload()` | — | `()` (hot-reload all) |

#### PolicyEngine

| Method | Input | Output |
|---|---|---|
| `evaluate(ctx, plan, budget)` | `DecisionContext`, `ExecutablePlan`, `CognitiveBudget` | `PolicyEvaluation` |
| `evaluate_rule(rule, ctx)` | `PolicyRule`, `DecisionContext` | `bool` |
| `check_guard_rails(plan, budget)` | `ExecutablePlan`, `CognitiveBudget` | `Vec<ViolatedRule>` |
| `enforce(evaluation, ctx)` | `PolicyEvaluation`, `&mut DecisionContext` | `()` (applies constraints to ctx) |

#### PolicyEvaluator

| Method | Input | Output |
|---|---|---|
| `applicable(policies, ctx)` | `Vec<RuleSet>`, `DecisionContext` | `Vec<PolicyRule>` |
| `evaluate(policy, ctx, plan)` | `RuleSet`, `DecisionContext`, `ExecutablePlan` | `PolicyEvaluation` |
| `aggregate(evaluations)` | `Vec<PolicyEvaluation>` | `PolicyEvaluation` |

#### PolicyCompiler

| Method | Input | Output |
|---|---|---|
| `compile(ruleset)` | `RuleSet` | `CompiledRuleSet` |
| `validate(compiled)` | `CompiledRuleSet` | `ValidationResult` |
| `optimize(compiled)` | `CompiledRuleSet` | `CompiledRuleSet` |

#### PolicyLoader

| Method | Input | Output |
|---|---|---|
| `load_from_config(config)` | `PolicyConfig` | `Vec<RuleSet>` |
| `load_from_file(path)` | `Path` | `RuleSet` |
| `watch_for_changes(path)` | `Path` | `Receiver<PolicyChange>` |

#### GuardRails

| Method | Input | Output |
|---|---|---|
| `active_guard_rails()` | — | `&[GuardRail]` |
| `check(plan, budget)` | `ExecutablePlan`, `CognitiveBudget` | `Vec<ViolatedRule>` |
| `reload(guard_rails)` | `Vec<GuardRail>` | `()` (swap in new rules) |

### Constraints

- `brain-policy` depends only on `brain-core`. No dependency on any other Brain crate.
- Policy rules are data (JSON / TOML); hot-reloadable without restarting the cognitive loop.
- `GuardRails::reload` is called on the `policy_reloaded` event; in-flight plans use the guard-rail snapshot valid at plan creation time.
- A `blocking` severity violation in any applicable rule causes `PolicyEvaluation::passed == false`.

---

## brain-reflection — Post-Execution Analysis

### Responsibility

Compare expected vs actual outcomes. Detect and classify mistakes. Generate improvements. Produce and persist lessons.

### Inputs

- `Decision` (expected outcome embedded in selected option)
- `ObservedOutcome` (actual result, from Observer or Execution Platform)
- `CognitiveBudget`

### Outputs

- `Reflection` (match_score, deviations, mistakes, improvements, timestamp)
- `Lesson` persisted to Memory Platform (semantic tier)
- Events: `ReflectionCompleted`, `OutcomeObserved`

### Key Trait: Reflector

| Method | Input | Output |
|---|---|---|
| `reflect(decision, actual_outcome)` | `Decision`, actual outcome string | `Reflection` |
| `compare(expected, actual)` | expected string, actual string | `OutcomeComparison` with `match_score [0.0..1.0]` |
| `score_match(comparison)` | `OutcomeComparison` | `f64` (0.0–1.0) |

### Key Trait: MistakeDetector

| Method | Input | Output |
|---|---|---|
| `detect(reflection)` | `Reflection` | `Vec<Mistake>` |
| `classify(mistake)` | `Mistake` | `MistakeType` |
| `severity(mistake)` | `Mistake` | `DeviationSeverity` |

### Key Trait: ImprovementGenerator

| Method | Input | Output |
|---|---|---|
| `generate(reflection, lessons)` | `Reflection`, `Vec<Lesson>` | `Vec<Improvement>` |
| `prioritize(improvements)` | `Vec<Improvement>` | `Vec<Improvement>` (sorted by benefit × confidence × criticality) |
| `apply(improvement)` | `Improvement` | `()` (updates planner heuristics or reasoner config) |

### Key Trait: LessonStore

| Method | Input | Output |
|---|---|---|
| `store(lesson)` | `Lesson` | `LessonId` (persisted to Memory Platform) |
| `retrieve(context)` | context string | `Vec<Lesson>` |
| `apply_to_planning(lessons)` | `Vec<Lesson>` | `PlanningAdjustments` (strategy_bias, step_ordering_hints) |

### Match Score Threshold

If `match_score < 0.8`: deviations are classified by dimension. Each deviation is assigned a `DeviationSeverity`: Negligible / Minor / Major / Critical. Mistakes are typed: PlanningError / ReasoningError / DecisionError / ExecutionError / PerceptionError / DataError.

---

## brain-workflow — Multi-Step Orchestration

### Responsibility

Orchestrate multi-step workflows with checkpoint/recovery. Steps reference abstract `ToolRequirement` objects. Execution Platform (Phase 8) maps these to actual dispatch.

### Inputs

- `Workflow` definition (steps, edges, composition type)
- `ToolRegistry` (for step tool selection)
- Runtime task events (`task.completed`, `task.failed`)

### Outputs

- Step status updates
- Checkpoint persistence (to Memory Platform)
- Events: `WorkflowStarted`, `WorkflowStepStarted`, `WorkflowStepCompleted`, `WorkflowCheckpointed`, `WorkflowCompleted`, `WorkflowFailed`

### Key Trait: WorkflowEngine

| Method | Input | Output |
|---|---|---|
| `start(workflow)` | `Workflow` | `WorkflowId` |
| `pause(workflow_id)` | `WorkflowId` | `()` |
| `resume(workflow_id)` | `WorkflowId` | `()` |
| `cancel(workflow_id)` | `WorkflowId` | `()` |
| `status(workflow_id)` | `WorkflowId` | `WorkflowStatus` |

### Composition Primitives

| Primitive | Behaviour |
|---|---|
| Sequential | Steps execute in declared order |
| Parallel | Steps with no mutual dependency run concurrently |
| Conditional | Branch on `DecisionContext` result |
| FanOut | Broadcast to N branches; all must return |
| FanIn | Gather from N branches; proceed when all complete |
| TryCatch | On step failure: run error handler; continue or abort based on policy |

### Checkpointing

- Automatic checkpoints every `checkpoint_interval_secs` (configurable).
- Serialised state size limited to `checkpoint_max_size_bytes`.
- Ring buffer: max `max_checkpoints_per_workflow` checkpoints per workflow. Oldest evicted on overflow.

---

## brain-learning — Memory Coordination

### Responsibility

Coordinate with Memory to promote knowledge. Feed improvements back to Planner and Reasoner. **No ML training.**

### Inputs

- `ReflectionCompleted` event (from `brain-reflection`)
- `GoalCompleted` event
- `Mistake` events
- Periodic timer or `PerformanceDegraded` signal

### Outputs

- `SemanticFact` written to Memory Platform
- `PlanningAdjustments` emitted to `brain-planner`
- Heuristic weight updates sent to `brain-reasoner`
- Events: `LearningCycleStarted`, `LearningCycleCompleted`, `PromotionApplied`

### Key Trait: Learner

| Method | Input | Output |
|---|---|---|
| `request(request)` | `LearningRequest` | `LearningResult` |
| `on_reflection(reflection)` | `Reflection` | `()` |
| `on_goal_completed(goal_id)` | `GoalId` | `()` |
| `on_mistake(mistake)` | `Mistake` | `()` |

### Memory Promotion Criteria

`should_promote()` weights:

- Recency: `memory_access_recency_weight` (default 0.5)
- Frequency: `memory_access_frequency_weight` (default 0.3)
- Importance: `memory_importance_weight` (default 0.2)

A memory item must score ≥ `promotion_threshold` (default 0.8) to be promoted from episodic to semantic.

---

## brain-coordinator — Orchestrator

### Responsibility

Runs the primary cognitive loop. Owns `BrainState` FSM. Enforces `CognitiveBudget`. Wires all subsystems. Publishes and subscribes to all Brain and cross-layer events.

### Inputs

- `GoalCreated` event
- `BrainPaused`, `BrainResumed`, `BrainStopping` events
- `PolicyReloaded`, `BudgetExhausted`, `ModelProviderUnavailable` events
- Operator commands (pause, resume, cancel, override)

### Outputs

- All Brain events (delegated to respective crates)
- `OrchestrationResult` per tick: action taken, reason, duration

### Key Trait: CognitiveLoop

| Method | Input | Output |
|---|---|---|
| `tick()` | — | `OrchestrationResult` (one cognitive cycle for current goal) |
| `run_until_idle(max_iterations)` | max iterations | `Vec<OrchestrationResult>` |
| `pause()` | — | `()` |
| `resume()` | — | `()` |
| `state()` | — | `BrainState` |
| `inject_goal(goal)` | `Goal` | `()` |
| `on_event(event)` | `dyn Event` | `()` (reactive handler for cross-layer events) |

### Key Trait: Coordinator (Accessor)

Returns `&dyn Trait` references to all subsystems:

| Method | Returns |
|---|---|
| `goals()` | `&dyn GoalManager` |
| `planner()` | `&dyn Planner` |
| `reasoner()` | `&dyn Reasoner` |
| `decision_maker()` | `&dyn DecisionMaker` |
| `reflector()` | `&dyn Reflector` |
| `learner()` | `&dyn Learner` |
| `workflow_engine()` | `&dyn WorkflowEngine` |
| `policy_engine()` | `&dyn brain_policy::PolicyEngine` |
| `model_registry()` | `&dyn brain_model::ModelProviderRegistry` |

### BrainState FSM

Coordinator is the **sole owner** of the current `BrainState`. All transitions are validated by `BrainStateMachine::validate_transition(current, new)`.

| From | To | Trigger |
|---|---|---|
| Sleeping | Idle | Start invoked |
| Idle | Planning | Active goal available |
| Planning | Reasoning | Plan created |
| Reasoning | WaitingModel | Model call dispatched |
| WaitingModel | Planning | Model returned, continue loop |
| WaitingModel | WaitingExecution | Plan selected, dispatch tool |
| WaitingExecution | Reflecting | Tool result received |
| Reflecting | Learning | Reflection complete |
| Learning | Idle | Memory updated |
| * | Recovering | Unrecoverable error |
| Recovering | Idle | Recovery plan executed |
| * | Paused | Pause request |
| Paused | Idle | Resume request |
| * | Stopping | Stop invoked |
| Stopping | Stopped | Persist complete |

---

## brain-model — Provider-Neutral Model Abstraction

### Responsibility

Abstract all AI model providers behind traits. No Brain crate imports a specific model SDK.

### Inputs

- `ModelConfig` (provider name, model name, parameters, timeout)
- Prompt messages with trace context

### Outputs

- `Completion` (content, usage, finish_reason, timestamp)
- `Embedding`, `TokenCount` as needed

### Key Traits

| Trait | Responsibility |
|---|---|
| `ModelProvider` | Provider registration, health check, completion |
| `ReasoningModel` | Chain-of-thought, inference, hypothesis generation |
| `PlanningModel` | Task graph generation, plan alternatives |
| `EmbeddingProvider` | Vector embeddings for memory |
| `TokenCounter` | Token counting without full completion |
| `PromptRenderer` | Template rendering for structured prompts |
| `ResponseParser` | Parse raw model output into typed structs |
| `ConversationContext` | Message history management |
| `ModelProviderRegistry` | Register, resolve, and route to providers |

### Constraints

- `brain-model` depends only on `brain-core`. No provider SDK imports.
- Implementations are loaded at startup and on `model.provider.available` events.
- Circuit breaker: after `circuit_breaker_failure_threshold` failures, a provider is marked `Unavailable` for `circuit_breaker_recovery_secs`.
- A `fallback_provider` is used when the primary is unavailable.

---

## Events

See [brain-events.md](brain-events.md) for the full canonical event catalog. Summary:

### Published by Brain

| Event | Publisher | Trigger |
|---|---|---|
| `brain.goal.created` | GoalManager | New goal registered |
| `brain.goal.activated` | GoalManager | Dependencies satisfied |
| `brain.goal.completed` | GoalManager | All steps succeeded |
| `brain.goal.failed` | GoalManager | Replan/recovery exhausted |
| `brain.goal.cancelled` | GoalManager | Operator cancel |
| `brain.plan.created` | Planner | New plan generated |
| `brain.plan.rejected` | PolicyEngine | Policy blocked plan |
| `brain.reasoning.started/finished` | Reasoner | Reasoning cycle |
| `brain.decision.made` | DecisionMaker | Action selected |
| `brain.decision.rejected` | DecisionMaker | Policy rejected |
| `brain.reflection.completed` | Reflector | Reflection done |
| `brain.learning.cycle.started/completed` | Learner | Learning cycle |
| `brain.workflow.started/completed` | WorkflowEngine | Workflow lifecycle |
| `brain.budget.exhausted` | Coordinator | Budget dimension exhausted |
| `brain.paused/resumed/stopped/failed` | Coordinator | Lifecycle transitions |

### Consumed by Brain

| Event | Source | Consumer |
|---|---|---|
| `memory.item.retrieved` | Memory | Reasoner, GoalManager |
| `task.completed` | Runtime | WorkflowEngine, Reflector |
| `task.failed` | Runtime | WorkflowEngine, Reflector |
| `execution.result.available` | Execution (future) | Reflector |
| `perception.observation.new` | Perception (future) | Reasoner |
| `policy.rule.changed` | brain-policy | PolicyEngine (hot-reload) |
| `operator.command` | External UI | Coordinator |

---

## Concurrency Model

- Brain has no dedicated background threads. The cognitive loop runs on the Coordinator's async task.
- All I/O (model calls, Memory reads/writes) is async (Tokio).
- CPU-bound work uses `tokio::task::spawn_blocking`.
- Lock acquisitions are scope-contained and always dropped before any `.await`.
- `std::sync::RwLock` used throughout (Brain never holds locks across `.await`).
- `Arc<dyn Trait>` for all shared trait objects inside Coordinator.
- Pure functions preferred (hypothesis evaluation, confidence scoring, utility computation).

---

## Determinism Guarantees

| Operation | Deterministic? | Notes |
|---|---|---|
| Goal CRUD | Yes | Lock-protected |
| Dependency checking | Yes | Pure graph algorithm |
| Plan generation (structure) | Partially | Model calls are non-deterministic; graph shape is deterministic |
| Decision selection | Yes | Same inputs → same output |
| Policy evaluation | Yes | Same rules, same inputs → same result |
| Reflection comparison | Yes | Deterministic field comparison |
| Lesson storage | Yes | Idempotent by lesson_id |
| Workflow step ordering | Yes | Dependency graph determines order |
| Non-deterministic boundary | `brain-model` only | All model invocations are non-deterministic by nature |

---

## Error Handling

### Error Hierarchy: BrainError

| Variant | Recovery | Consumed By |
|---|---|---|
| `GoalNotFound(GoalId)` | Log; skip tick | Coordinator |
| `PlanNotFound(PlanId)` | Log; trigger replan | Coordinator |
| `DecisionNotFound(DecisionId)` | Log; escalate | Human |
| `WorkflowNotFound(WorkflowId)` | Log; skip | WorkflowEngine |
| `InvalidStateTransition(from, to)` | Log; halt cognitive loop | Coordinator → Stopping |
| `CycleDetected(GoalIds)` | Emit `GoalDependencyViolation`; operator resolves | GoalManager |
| `DependencyViolation(blocker, blocked)` | Block until dependency met | GoalManager |
| `ConstraintViolation(detail)` | Policy rejection | DecisionMaker |
| `PolicyViolation(detail)` | Replan within constraints; escalate if blocking | DecisionMaker |
| `Conflict(detail)` | Escalate or auto-resolve per config | DecisionMaker |
| `BudgetExhausted(dimension)` | Degrade budget; re-run phase | All phases |
| `ModelProviderError(detail)` | Retry (2); on exhaustion: cached/default reasoning | Reasoner, Planner |
| `NoModelAvailable(capability)` | Replan without that capability | Planner |
| `ToolNotFound(capability)` | Replan without unavailable tool | Planner |
| `MemoryError(detail)` | Pause loop; retry storage with backoff; if persistent → Stopping | Learner, GoalManager |
| `ValidationError(detail)` | Log; return error to caller | All |
| `Internal(detail)` | Log; escalate to operator | All |
| `LockPoisoned(lock_name)` | Log; platform restart required | All |

### Recovery Strategies

| Failure | Strategy |
|---|---|
| Model call failure | Retry (2) with backoff; then emit `DegradedMode`, use cached/default reasoning |
| Planning failure | Fall back to simpler strategy (MEA → FC → BC → Template). If all fail: mark Goal Failed. |
| Tool not found | Replan without the unavailable tool requirement. |
| Budget exhaustion | Degrade budget (reduce branches, depth, model calls). Continue with reduced scope. |
| Policy rejection | Replan within constraints. If still rejected, emit `DecisionRejected` and escalate. |
| Dependency cycle | Emit `GoalDependencyViolation` event. Operator resolves via disambiguation. |
| Memory/storage failure | Pause CognitiveLoop. Retry storage with backoff. If persistent: Recovering → Stopping. |
| Unrecoverable error | Any phase → Recovering. if recovery fails after N attempts → Stopping → operator. |

---

## Lifecycle

`BrainPlatform` implements Core's `Service` trait.

### Start Order

```text
1. brain-policy ──► load PolicyConfig, rule sets, guard rails
2. brain-model ──► register default provider(s)
3. brain-goals ──► restore active goals from Memory
4. brain-reflection ──► restore recent lessons
5. brain-workflow ──► restore active workflows
6. brain-reasoner ──► load reasoning heuristics
7. brain-decision ──► load policy evaluators
8. brain-coordinator ──► wire all subsystems, subscribe events, set state: Idle
```

### Stop Sequence

```text
1. brain-coordinator ──► stop cognitive loop, set state: Stopping
2. brain-workflow ──► checkpoint all running workflows
3. brain-goals ──► persist active goals
4. brain-reflection ──► persist new lessons
5. brain-reasoner ──► persist plan cache (if any)
6. brain-policy ──► persist runtime policy state
7. All ──► unsubscribe events
8. brain-coordinator ──► set state: Stopped
```

---

## Performance Targets

| Operation | Target Latency | Notes |
|---|---|---|
| `GoalManager.create_goal()` | < 50 µs | HashMap + event emission |
| `Planner.decompose()` | < 10 ms | Without model call; model I/O is async |
| `Reasoner.reason()` | < 5 ms | Without model; model call I/O is async |
| `DecisionMaker.decide()` | < 500 µs | Policy evaluation + utility scoring |
| `PolicyEngine.evaluate()` | < 200 µs | In-memory rule evaluation |
| `Reflector.reflect()` | < 1 ms | Field comparison + mistake detection |
| `Learner.on_reflection()` | < 5 ms | Memory queries + promotion queue |

---

## Future Extensions

- **Meta-Cognition**: A monitoring loop that evaluates the reasoning engine's own performance and adjusts strategies.
- **Multi-Brain Negotiation**: Multiple Brain instances negotiate goal priority and resource allocation via structured protocol.
- **Case-Based Reasoning**: Match new goals against a library of successful plans to reuse or adapt prior solutions.
- **Probabilistic Reasoning**: Replace single-confidence values with probability distributions.
- **Knowledge Graph Persistence**: Snapshot and restore the in-memory graph to Memory Platform Long-Term Storage.
- **Tool Registry Federation**: Allow Execution Platform to register custom tools at runtime.

---

## References

- [Architecture Overview](overview.md)
- [Core Platform Deep Dive](core.md)
- [Runtime Platform Deep Dive](runtime.md)
- [Memory Platform Architecture](memory.md)
- [System Platform (OSAL)](system.md)
- [Design Principles](../principles.md)
- [Roadmap](../roadmap.md)
- [Cognitive Loop Detail](cognitive-loop.md)
- [Event Catalog](brain-events.md)
- [Dependency Matrix](brain-dependencies.md)
- [Configuration Reference](configuration/brain.md)
- [RFC-0003: Brain Platform](../rfc/RFC-0003-brain-platform.md)
