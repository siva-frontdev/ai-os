# RFC-0003: Brain Platform (Revised)

| Field | Value |
|---|---|
| **Status** | Accepted — Revised |
| **Author** | Project maintainers |
| **Phase** | Phase 6 |
| **Created** | 2025-02-15 |
| **Updated** | 2026-01-XX |
| **Requires** | RFC-0001 (System Platform), RFC-0002 (Memory Platform) |
| **Supersedes** | RFC-0003 (original draft) |

## Abstract

This RFC proposes the Brain Platform for the AI-native OS, organized as Phase 6 of the implementation roadmap. The Brain Platform provides an integrated reasoning and decision-making layer that decomposes high-level goals into executable plans using abstract tool requirements, evaluates outcomes in a planner-executor-evaluator loop, selects between alternative actions using configurable decision strategies, manages a persistent goal DAG with priority and dependency tracking, maintains a knowledge graph of entities and relations backed by long-term memory, and recognizes intent from natural language or structured input. It depends on the Memory Platform (Phase 5) for knowledge persistence and retrieval, the Runtime Platform (Phase 3) for task execution and scheduling, and Core (Phase 2) for EventBus and lifecycle. Five post-approval refinements — `brain-policy` crate extraction, `brain-reasoner` submodule reorganization, `CognitiveBudget`, abstract Tool Planning, and `BrainState` FSM — are incorporated into this revision. The Brain is not an LLM integration layer (that belongs to Phase 9, Intelligence Integration); it is the structured reasoning engine that operates on explicit knowledge representations to drive autonomous platform behavior.

---

## Motivation

An AI-native OS must do more than store and retrieve information; it must reason about goals, make decisions under uncertainty, and adapt its behavior based on outcomes. Without a dedicated Brain Platform, each agent or subsystem would implement ad-hoc planning logic, hardcoded decision policies, and bespoke goal tracking — leading to architectural drift, inconsistent audit trails, and the inability to coordinate across subsystems.

The AI Lifecycle defined in `specification.md` section 7 (Act -> Observe -> Learn -> Sleep) requires a structured reasoning substrate. The Act state demands a planner that can decompose goals into steps. The Observe state requires an evaluator that compares outcomes against expectations. The Learn state feeds observations back into the knowledge graph and goal manager.

The complete cognitive loop (Goal -> Plan -> Decide -> Execute -> Observe -> Reflect -> Learn -> Update Memory) must be a platform primitive, not an application concern. Every agent, from system management to user-facing interactions, relies on the same reasoning infrastructure.

### Specification Cross-References

- `specification.md` section 7 (AI Lifecycle): The Brain implements the Act->Observe->Learn loop.
- `specification.md` section 12 (Module Contracts): The Brain follows all module contract requirements (Service trait, EventBus, HealthMonitor).
- `specification.md` section 4 (Security Goals): Policy gate before every decision; all reasoning is logged for audit.
- `docs/architecture/brain.md`: Refined architecture with all post-approval changes.
- `docs/cognitive-loop.md`: Detailed guidance for the cognitive loop, state machines, budget, and tool planning.

---

## Design

### Overview

The Brain Platform is composed of ten sub-crates, each with a distinct responsibility:

```
┌──────────────┐
│ brain-core │
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
│ +      │ │lec-│ │ +     ││        ││       │
│tools   │ │tion │ │policy ││        ││       │
└───┬────┘ └──┬──┘└───┬────┘└───┬────┘└───┬───┘
       │ │ │ │ │
┌────┴────────┴───────┴──────────┴────────┐
│ brain-coordinator │ ← orchestrator
└──────────────────────────────────────────┘

brain-policy (standalone; consumed by decision + coordinator)
```

### Status of Core Subsystems

| Subsystem | Location | Status |
|---|---|---|
| Goal management | `brain-goals` | Defined |
| Planning | `brain-planner` | Defined |
| Reasoning | `brain-reasoner` | Defined |
| Decision | `brain-decision` | Defined |
| Policy | `brain-policy` | New |
| Reflection | `brain-reflection` | Defined |
| Workflow | `brain-workflow` | Defined |
| Learning | `brain-learning` | Defined |
| Coordination | `brain-coordinator` | Defined |
| Model abstraction | `brain-model` | Defined |

All ten subsystems are wired together by `Coordinator`, which runs the cognitive loop.

---

### Subsystem 1: Reasoning Engine (brain-reasoner)

The Reasoning Engine generates structured reasoning from observations, context, and memory. It is organized into internal submodules, each defining a trait with a Default implementation. All submodule traits are Send + Sync + Debug.

**Submodules (all within `brain-reasoner/src/`):**

- `reasoning/` — Primary entry point; orchestration
- `inference/` — Chain-of-thought generation, inference generation
- `constraints/` — CSP-style constraint checking
- `risk/` — Probabilistic risk assessment
- `tradeoff/` — Pareto frontier, multi-criteria ranking
- `hypothesis/` — Hypothesis generation and evaluation

**Top-level Reasoner trait:**
- `reason(ctx, budget)` — full reasoning cycle within budget → ReasoningResult
- `generate_hypotheses(observations)` → Vec<Hypothesis>
- `evaluate(hypothesis, evidence)` → Confidence
- `select_best_hypothesis(hypotheses)` → Option<Hypothesis>

`ReasoningEngine` delegates to the submodules. No new crates created by this reorganization.

---

### Subsystem 2: Planner (brain-planner)

The Planner converts goals into ExecutablePlan structs containing abstract ToolRequirement objects — not OS commands. The ToolSelector sub-trait (within brain-planner) matches ToolRequirement to ToolCandidate via a ToolRegistry. Execution Platform (Phase 8) will map ToolCandidate to actual dispatches.

**Planner flow:**
1. `decompose(goal, budget)` called with goal and CognitiveBudget
2. Acquire brief write lock on goal, call model: `generate_task_graph(goal, context)`
3. `TaskGraphBuilder::resolve_dependencies(graph)` — topological sort; cycles return error
4. Select planning strategy by budget priority: MeansEnds (≥3), ForwardChaining (≥2), BackwardChaining (≥1), TemplateMatch (else)
5. For each strategy (up to budget.max_branches): generate AlternativePlan, estimate cost/duration/confidence
6. `Planner::select_best(alternatives, criteria)` — minimise cost, maximise confidence
7. Convert plan actions to Vec<ToolRequirement> (abstract, not OS commands)
8. `PlanOptimizer::optimize` if budget allows: prune, merge, reorder, revalidate budget
9. Return ExecutablePlan to Coordinator

---

### Subsystem 3: Policy Engine (brain-policy) — New

Policy evaluation is extracted from brain-decision into its own crate. This separation allows policy rules to be loaded, compiled, and hot-reloaded independently of the decision algorithm.

**Key behavioral contract:** The PolicyEngine is consulted before every decision. A PolicyEvaluation::passed == false with a blocking severity prevents the plan from proceeding. The DecisionMaker never selects an action from a blocked plan.

**Traits defined in brain-policy:**

- `PolicyStore` — load/store/list rulesets, load guard rails, reload (hot-reload all)
- `PolicyEngine` — evaluate(ctx, plan, budget) → PolicyEvaluation; check_guard_rails; enforce
- `PolicyEvaluator` — applicable(policies, ctx) → matching rules; evaluate(policy, ctx, plan) → PolicyEvaluation; aggregate(evaluations) → PolicyEvaluation
- `PolicyCompiler` — compile(ruleset) → CompiledRuleSet; validate; optimize
- `PolicyLoader` — load_from_config, load_from_file, watch_for_changes (returns Receiver for hot-reload)
- `GuardRails` — active_guard_rails(), check(plan, budget) → Vec<ViolatedRule>, reload(new_guard_rails)

**Policy evaluation result fields:** passed (bool), violated_rules (Vec<ViolatedRule>), warnings (Vec<String>), allowance (f64: 0.0 = fully blocked, 1.0 = fully allowed).

**Rule types:** Always, ContextMatches, Threshold, BudgetExceeded, GuardRailTriggered.

**Actions:** Allow, Block, AdjustWeight, Escalate, Log.

---

### Subsystem 4: Decision Maker (brain-decision)

Policy-conscious decision selection with confidence scoring and explanation generation. Consumes brain-policy.

**Decision flow:**
1. `PolicyEngine::evaluate(plan, policies)` → PolicyEvaluation
2. If rejected: emit DecisionRejected, return error
3. ConfidenceScorer → rank_options()
4. ConflictResolver → detect + resolve conflicts
5. select_action() → Decision
6. ExplanationGenerator::explain() → DecisionExplanation
7. Emit DecisionMade event

**Traits:**
- `DecisionMaker::decide(ctx, budget)` → Decision (calls PolicyEngine first)
- `DecisionMaker::select_action(options, criteria)` → DecisionOption
- `DecisionMaker::rank_options(options)` → Vec<(DecisionOption, f64)>
- `ExplanationGenerator::explain(decision)` → DecisionExplanation

---

### Subsystem 5: Goal Manager (brain-goals)

Goals form a DAG (not tree). A goal can have multiple parents. Dependencies use Vec<GoalDependency> with typed blocking semantics.

**Goal lifecycle:**
Pending → Active → Planning → Executing → Evaluating → Completed
     also: Evaluating → Recovering → Executing
     also: any → Paused (cooperative pause at phase boundaries)
     also: any → Cancelled (operator cancel)

**Transition rules:**
- Pending → Active: dependencies satisfied, no cycles
- Active → Planning: Planner invoked
- Planning → Executing: Plan approved by PolicyEngine
- Executing → Evaluating: Step completed
- Evaluating → Completed: all steps succeed
- Evaluating → Failed: replan exhausted
- Evaluating → Recovering: recoverable error
- Recovering → Executing: recovery plan ready
- * → Paused: operator pause request (cooperative)
- * → Cancelled: operator cancel request

**Traits:**
- `GoalManager::create(goal)` → GoalId (emits GoalCreated)
- `GoalManager::activate(goal_id)` → () (validates deps; emits GoalActivated)
- `GoalManager::complete(goal_id, outcome)` → () (emits GoalCompleted)
- `GoalManager::fail(goal_id, error)` → () (emits GoalFailed)
- `GoalManager::cancel(goal_id, reason)` → () (emits GoalCancelled)
- `GoalManager::status(goal_id)` → GoalStatus
- `GoalValidator::validate(goal)` → ValidationResult
- `GoalValidator::validate_dependencies(id)` → Vec<GoalDependencyViolation>
- `GoalValidator::check_cycles()` → Vec<Vec<GoalId>>

---

### Subsystem 6: Reflection (brain-reflection)

Post-execution analysis comparing expected vs actual outcomes, classifying mistakes, generating improvements, and producing lessons.

**Flow:**
1. Triggered by Reflector::reflect(decision, actual_outcome) after execution
2. Compares expected vs actual → OutcomeComparison with match_score [0.0..1.0]
3. If match_score < 0.8: detect deviations, classify severity: Negligible / Minor / Major / Critical
4. MistakeDetector::detect(reflection) → Vec<Mistake> classified by MistakeType
5. ImprovementGenerator::generate(reflection, mistakes) → Vec<Improvement>
6. Prioritize improvements: estimated_benefit × confidence × target criticality
7. LessonStore::store(lesson) → LessonId persisted to Memory Platform semantic tier
8. Emit ReflectionCompleted event

**Traits:**
- `Reflector::reflect(decision, actual_outcome)` → Reflection
- `Reflector::compare(expected, actual)` → OutcomeComparison
- `Reflector::score_match(comparison)` → f64
- `MistakeDetector::detect(reflection)` → Vec<Mistake>
- `MistakeDetector::classify(mistake)` → MistakeType
- `MistakeDetector::severity(mistake)` → DeviationSeverity
- `ImprovementGenerator::generate(reflection, lessons)` → Vec<Improvement>
- `ImprovementGenerator::prioritize(improvements)` → Vec<Improvement>
- `LessonStore::store(lesson)` → LessonId
- `LessonStore::retrieve(context)` → Vec<Lesson>
- `LessonStore::apply_to_planning(lessons)` → PlanningAdjustments

---

### Subsystem 7: Learning (brain-learning)

Coordinate with Memory to identify knowledge worth promoting, consolidate episodic memory into semantic facts, and feed improvements back to the Planner and Reasoner. No ML training.

**Learning triggers:**
- GoalCompleted (from GoalManager)
- Mistake detected (from Reflector)
- PerformanceDegraded (from Metrics)
- Periodic timer (configurable interval)

**Flow:**
1. Identify candidates: MemoryPromoter::identify_candidates(scope) → Vec<EpisodicMemoryRef>
2. Filter by should_promote(): criteria are access_count, importance, recency, pattern recurrence
3. Consolidate: MemoryPromoter::consolidate(candidates) → Vec<SemanticFact> (written to MemoryPlatform)
4. Feed back to planner: PlanningAdjustments { strategy_bias, step_ordering_hints } emitted to brain-planner
5. Feed back to reasoner: updated heuristic weights sent to brain-reasoner
6. Emit LearningCompleted event

**Traits:**
- `Learner::request(request)` → LearningResult
- `Learner::on_reflection(reflection)` → ()
- `Learner::on_goal_completed(goal_id)` → ()
- `Learner::on_mistake(mistake)` → ()

---

### Subsystem 8: Workflow (brain-workflow)

Long-running multi-step orchestration with checkpoint/recovery. Supports sequential, parallel, conditional, fan-out/fan-in, and try-catch composition patterns. Steps reference abstract ToolRequirement objects — no OS commands. Execution Platform (Phase 8) maps these to actual dispatch.

**Composition primitives:**
- Sequential: steps execute in declared order
- Parallel: steps with no mutual dependency run concurrently (fan-out)
- Conditional: branch on DecisionContext result
- FanOut/FanIn: broadcast + gather pattern
- TryCatch: on step failure, run error handler; continue or abort based on policy

**Checkpointing:** Automatic checkpoints every `checkpoint_interval_secs` (configurable). Serialised state size limited to `checkpoint_max_size_bytes`. Ring buffer: max `max_checkpoints_per_workflow` per workflow.

**Traits:**
- `WorkflowEngine::start(workflow)` → WorkflowId
- `WorkflowEngine::pause/resume/cancel/status` → ()
- `CheckpointManager::create_checkpoint(workflow_id)` → CheckpointId
- `CheckpointManager::restore_checkpoint(checkpoint_id)` → Result<WorkflowState>
- `WorkflowComposer::compose(steps, composition)` → Workflow

---

### Subsystem 9: Coordinator (brain-coordinator)

The Coordinator owns: CognitiveState (current goal, plan, decision, reflection, budget, state), BrainState FSM, CognitiveBudget enforcement, the cognitive loop (tick()), event subscriptions, and subsystem access (all &dyn Trait).

**Startup sequence:**
1. Load policy rulesets and guard rails
2. Register model providers
3. Restore active goals from Memory
4. Restore recent lessons
5. Restore active workflows
6. Load reasoning heuristics
7. Construct DecisionMaker with PolicyEngine
8. Wire all subsystems, subscribe events, set state: Idle

**Key traits:**

- `CognitiveLoop::tick()` — one cognitive cycle for current goal → OrchestrationResult
- `CognitiveLoop::run_until_idle(max_iterations)` → Vec<OrchestrationResult>
- `CognitiveLoop::pause/resume/state/inject_goal/on_event` — lifecycle and injection
- `Coordinator::goals()/planner()/reasoner()/decision_maker()/reflector()/learner()/workflow_engine()/policy_engine()/model_registry()` — subsystem accessors

---

### Subsystem 10: Model Abstraction (brain-model)

Provider-neutral. The Brain never imports a provider SDK.

**Key traits:**
- `ModelProvider` — provider registration, health check, completion
- `ReasoningModel` — chain-of-thought, inference, hypothesis generation
- `PlanningModel` — task graph generation, plan alternatives
- `EmbeddingProvider` — vector embeddings for memory
- `TokenCounter` — token counting without full completion
- `PromptRenderer` — template rendering for structured prompts
- `ResponseParser` — parse raw model output into typed structs
- `ConversationContext` — message history management
- `ModelProviderRegistry` — register, resolve, and route to providers

**Contract:**
- Implementations are loaded at startup and on `model.provider.available` events.
- Circuit breaker: after `circuit_breaker_failure_threshold` failures, a provider is marked Unavailable for `circuit_breaker_recovery_secs`.
- A `fallback_provider` is used when the primary is unavailable.

---

## CognitiveBudget

The central resource management primitive for reasoning cycles. Every cycle receives a CognitiveBudget before entering the loop. Budget consumption is tracked in BudgetUsage and checked at each phase boundary. Budget exhaustion triggers degradation (reduced scope) rather than hard failure.

**Fields:**
- max_thinking_time_ns (u64) — maximum wall-clock per cycle
- max_iterations (usize) — loop iterations before forced termination
- max_reasoning_depth (usize) — chain-of-thought steps
- max_branches (usize) — parallel reasoning branches
- max_model_calls (usize) — LLM invocations per cycle
- max_token_budget (u32) — tokens across all model calls
- max_cost_budget_cents (u64) — USD cents ceiling per cycle
- deadline (CognitiveTimestamp) — absolute deadline
- priority (u8) — relative priority

**Budget usage tracked fields:** iterations_used, reasoning_depth_reached, branches_spawned, model_calls_used, tokens_consumed, cost_incurred_cents, started_at, last_checkpoint.

**BudgetChecker contract:** exhausted(usage) → bool, remaining_tokens(usage) → u32, remaining_cost_cents(usage) → u64, past_deadline(now) → bool, degraded() → reduced CognitiveBudget.

**Budget enforcement by phase:**
- Planning: checks iterations, token budget; on exhaustion: degrade (fewer alternatives)
- Reasoning: checks max_reasoning_depth, max_model_calls; on exhaustion: pause, emit BudgetExhausted
- Decision: always passes (cheap)
- Tool Planning: checks max_branches; prune lowest-value branches
- Reflection: checks iterations; write partial reflection, emit warning
- Learning: checks cost budget; defer to next cycle

---

## Tool Planning

The Planner produces abstract ToolRequirement specifications. Execution Platform (Phase 8) maps these to actual OS commands or service calls.

**ToolRequirement declares what capability is needed, not which tool executes it:**
- capability: ToolCapabilityId (e.g., "read_file", "run_command")
- inputs: HashMap<String, ToolValue> (abstract values, not OS handles)
- expected_outputs: Vec<String> (named outputs expected back)
- timeout_ms: Option<u64>
- retry_policy: RetryPolicy
- priority: u8

**ToolCandidate satisfies a ToolRequirement:**
- provider: String ("execution", "perception", "custom:foo")
- capability: ToolCapabilityId (matches requirement)
- estimated_cost, estimated_duration_ms, confidence, side_effects (Vec<SideEffect>)

**SideEffect types:** FilesystemWrite, NetworkRequest, ProcessSpawn, StateMutation, NoSideEffects.

**Selection flow:** Planner produces ToolRequirements → ToolSelector matches via ToolRegistry, ranks by confidence/cost/latency, selects best → DecisionMaker validates side effects and checks policies → approved ToolCandidate returned. Future: Execution Platform dispatches.

---

## BrainState

Coordinator owns the state. All transitions are validated through a state machine.

**States (unit variants):** Sleeping, Idle, Planning, Reasoning, WaitingModel, WaitingExecution, Reflecting, Learning, Recovering, Paused, Stopped.

**Key transitions:**
- Sleeping → Idle (Start invoked)
- Idle → Planning (Active goal available)
- Planning → Reasoning (Plan created)
- Reasoning → WaitingModel (Model call dispatched)
- WaitingModel → Planning (Model returned, continue loop)
- WaitingModel → WaitingExecution (Plan selected, dispatch tool)
- WaitingExecution → Reflecting (Tool result received)
- Reflecting → Learning (Reflection complete)
- Learning → Idle (Memory updated)
- * → Recovering (Unrecoverable error)
- Recovering → Idle (Recovery plan executed)
- * → Paused (Pause request)
- Paused → Idle (Resume request)
- * → Stopping (Stop invoked)
- Stopping → Stopped (Persist complete)

---

## Error Handling

**BrainError variants:**

- GoalNotFound(GoalId) — log; skip tick
- PlanNotFound(PlanId) — log; trigger replan
- DecisionNotFound(DecisionId) — log; escalate
- WorkflowNotFound(WorkflowId) — log; skip
- InvalidStateTransition(from, to) — log; halt cognitive loop → Stopping
- CycleDetected(GoalIds) — emit GoalDependencyViolation; operator resolves
- DependencyViolation(blocker, blocked) — block until dependency met
- ConstraintViolation(detail) — policy rejection
- PolicyViolation(detail) — replan within constraints; escalate if blocking
- Conflict(detail) — escalate or auto-resolve per config
- BudgetExhausted(dimension) — degrade budget; re-run phase
- ModelProviderError(detail) — retry (2); on exhaustion: cached/default reasoning
- NoModelAvailable(capability) — replan without that capability
- ToolNotFound(capability) — replan without unavailable tool
- MemoryError(detail) — pause loop; retry storage; if persistent → Stopping
- ValidationError(detail) — log; return error to caller
- Internal(detail) — log; escalate to operator
- LockPoisoned(lock_name) — log; platform restart required

**Recovery strategies by variant:**
- PlanningFailure: fall back to simpler strategy (MEA → FC → BC → Template). If all fail: mark Goal Failed.
- BudgetExhausted: degrade budget (reduce branches, depth). Continue with reduced scope.
- PolicyViolation: replan within constraints. If blocking, emit DecisionRejected and escalate.
- ModelProviderError: retry with backoff (2 attempts). On permanent failure, emit DegradedMode, use cached/default reasoning.
- ToolNotFound: replan without the unavailable tool requirement.
- GoalCycleDetected: reject dependency. Emit GoalDependencyViolation event. Operator resolves via disambiguation.

---

## Lifecycle

BrainPlatform implements Core's Service trait.

**Start order:**
1. brain-policy — load PolicyConfig, rule sets, guard rails
2. brain-model — register default provider(s)
3. brain-goals — restore active goals from Memory
4. brain-reflection — restore recent lessons
5. brain-workflow — restore active workflows
6. brain-reasoner — load reasoning heuristics
7. brain-decision — load policy evaluators
8. brain-coordinator — wire all subsystems, subscribe events, set state: Idle

**Stop sequence:**
1. brain-coordinator — stop cognitive loop, set state: Stopping
2. brain-workflow — checkpoint all running workflows
3. brain-goals — persist active goals
4. brain-reflection — persist new lessons
5. brain-reasoner — persist plan cache (if any)
6. brain-policy — persist runtime policy state
7. All — unsubscribe events
8. brain-coordinator — set state: Stopped

---

## Dependencies

**External (lower layers):**
- ai_os_core (Core, layer 2): EventBus, Service, Logger, LifecycleManager — used by all Brain crates
- ai_os_runtime (Runtime, layer 3): Scheduler, TaskManager, Supervisor, PermissionChecker — used by Workflow, Planner
- memory_core (Memory, layer 4): Types, Timestamp — used by all Brain crates
- memory_episodic, memory_semantic, memory_working, memory_context (Memory, layers 4–5): knowledge retrieval — used by Reasoner, Learner, GoalManager

**Internal (Brain crates, layer 6):** See `docs/architecture/brain-dependencies.md` for the full dependency matrix.

**Consumer direction only (Brain → Memory):**
- brain-learning: calls memory promotion APIs (episodic → semantic)
- brain-reflection: writes lessons to Memory semantic tier
- brain-goals: persists and restores goal state
- brain-workflow: checkpoints workflow state

---

## Thread Model

No dedicated background threads in Brain. The cognitive loop runs on the Coordinator's async task. All blocking I/O uses tokio::task::spawn_blocking.

**Lock strategy:**
- Goal store, Plan store — RwLock<HashMap<...>> (read-heavy, short critical sections, never held across await)
- Cognitive state — RwLock<CognitiveState> (single writer per tick)
- Policy rules — Arc<Vec<PolicyRule>> (immutable after load; zero-lock reads)
- Guard rails — Arc<RwLock<Vec<GuardRail>>> (hot-reloadable)
- Budget — RwLock<BudgetUsage> (updated synchronously within tick)
- Event dispatch — EventBus (async, non-blocking)
- Model calls — HTTP/gRPC (network I/O; no local lock)

---

## Security Considerations

- **Policy gates**: Every plan is evaluated by PolicyEngine before a decision is made. Guard rails are checked with every model call and tool requirement.
- **Audit**: Every decision is logged with rationale, candidate scores, and confidence. All Brain events carry trace context.
- **Session isolation**: Goals and Memory queries are scoped to their session. Cross-session knowledge requires explicit policy grant.
- **No attack surface**: Brain runs in-process. No network listeners.
- **Budget enforcement**: Resource exhaustion is a denial-of-service vector; CognitiveBudget prevents runaway model calls.

---

## Performance Targets

- GoalManager.create_goal() — target < 50 µs (lock-protected HashMap insert)
- Planner.decompose() — target < 10 ms (without model call; model I/O is async)
- Reasoner.reason() — target < 5 ms (without model; model I/O is async)
- PolicyEngine.evaluate() — target < 200 µs (in-memory rule evaluation)
- DecisionMaker.decide() — target < 500 µs (policy evaluation + utility scoring)
- Reflector.reflect() — target < 1 ms (field comparison + mistake detection)
- Learner.on_reflection() — target < 5 ms (memory queries + promotion queue)

---

## Drawbacks

1. **CognitiveBudget tuning**: Budget parameters require operational experience. Incorrectly tight budgets starve reasoning; too loose budgets allow runaway cost. Mitigation: start conservative, monitor exhaustions, adjust.
2. **Abstract tool planning indirection**: Returning ToolRequirement rather than OS commands adds a mapping step in Phase 8. Makes testing harder (must mock tool registry). Benefit: Brain is decoupled from Execution; swapping Execution implementations has zero Brain changes.
3. **Policy hot-reload complexity**: Guard rails must be re-evaluated on the fly without restarting the cognitive loop. Locking the guard rail store during reload must not block in-flight planning.
4. **Goal DAG cycle detection**: Must be O(V+E) on each validation. Placing this in the validate step (not the create step) prevents race conditions but shifts cost to planning time.

---

## Open Questions

1. **Budget parameter defaults**: What are sensible defaults for max_token_budget, max_cost_budget_cents, and max_model_calls per goal? Should they be per-session or per-goal? Proposal: per-goal; session-level cap divides among active goals equally.
2. **Policy rule format**: Should rules be written in a DSL, JSON, or Rust closures? JSON is simplest for operator editability; Rust closures give maximum power but require compilation. Proposal: JSON rules for operators; Rust impls for embedded policy authors.
3. **Tool registry source**: Who populates the ToolRegistry? Execution Platform on startup? Operator configuration? Brain self-discovery? Proposal: Execution Platform registers at startup; operators can augment via config.
4. **Failure recovery depth**: How many Recovery plans should the Coordinator attempt before escalating to a human? Proposal: max 2 recovery cycles per goal; then emit brain.failed for operator intervention.
5. **Human interruption design**: Should pause/resume be cooperative (check state at safe points) or preemptive (cancel in-flight model calls)? Proposal: cooperative — only pause at phase boundaries (Planning, Reflecting, Learning), never during a model call.

---

## Alternatives Considered

| Alternative | Reason for Rejection |
|---|---|
| Policy in brain-decision | Violates single-responsibility. Policy compilation, hot-reload, and rule evaluation are complex enough to deserve their own crate. |
| Monolithic reasoner | Violates single-responsibility. Strategy switching requires recompilation. Testing is harder. |
| OS commands in planner | Couples Brain to Execution layer prematurely. Abstract tool requirements preserve the boundary. |
| No CognitiveBudget | No resource cap leads to runaway LLM costs. Budget is the only mechanism that keeps the platform economically bounded. |
| Single BrainState enum | Goal-level state (GoalStatus) and platform-level state (BrainState) have different semantics. Mixing them would conflate goal lifecycle with platform lifecycle. |
| Flat goal list | Cannot express dependency DAGs. No progress propagation. Hard to track related goals. |
