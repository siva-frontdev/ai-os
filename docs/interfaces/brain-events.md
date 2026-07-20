# Brain Events — Canonical Event Catalog

All events emitted or consumed by Brain Platform crates. Events are the **only** inter-crate communication mechanism. No direct method calls across crate boundaries.

---

## Event Naming Convention

| Convention | Example |
|---|---|
| Namespace | `brain.*` for Brain-originated events |
| Cross-layer | `memory.*`, `runtime.*`, `core.*` for consumed events |
| Verb-first | `goal_created`, `reasoning_started`, `budget_exhausted` |
| Tense | Past tense for completed actions, present participle for in-progress |

---

## Emitted Events

### 1. Goal Lifecycle Events

#### `brain.goal.created`

| Field | Type | Description |
|---|---|---|
| `goal_id` | `GoalId` | Unique identifier of the new goal |
| `goal_type` | `string` | Categorisation (system, user, derived, recovery) |
| `priority` | `GoalPriority` | Low / Normal / High / Critical |
| `dependencies` | `Vec<GoalId>` | Goals that must complete first |
| `deadline` | `CognitiveTimestamp?` | Optional absolute deadline |
| `session_id` | `SessionId` | Originating session for isolation |
| `trace_id` | `TraceId` | Distributed-trace context |

**Published by:** `brain-goals` GoalManager  
**Trigger:** Operator or upstream component creates a new goal  
**Consumers:** `brain-coordinator` (enqueue), `brain-workflow` (conditionally)

---

#### `brain.goal.activated`

| Field | Type | Description |
|---|---|---|
| `goal_id` | `GoalId` | |
| `prior_status` | `GoalStatus` | `Pending` |
| `new_status` | `Active` | |

**Published by:** `brain-goals` GoalManager  
**Trigger:** `activate(goal_id)` — all dependencies satisfied  
**Consumers:** `brain-coordinator` (begin cognitive cycle for this goal)

---

#### `brain.goal.completed`

| Field | Type | Description |
|---|---|---|
| `goal_id` | `GoalId` | |
| `outcome` | `string` | Human-readable summary |
| `result` | `GoalResult` | Structured success/failure data |
| `final_confidence` | `Confidence` | Confidence in the final outcome |
| `total_duration_ms` | `u64` | Wall-clock time from activation to completion |

**Published by:** `brain-goals` GoalManager  
**Trigger:** Evaluating phase confirms all steps succeeded  
**Consumers:** `brain-learning` (trigger learning cycle), `brain-coordinator` (dequeue)

---

#### `brain.goal.failed`

| Field | Type | Description |
|---|---|---|
| `goal_id` | `GoalId` | |
| `reason` | `string` | Failure description |
| `recovery_attempts` | `u32` | Number of recovery cycles tried |
| `last_error` | `BrainError` | The terminal error variant |
| `failed_at_phase` | `BrainPhase` | Which phase failed (Planning, Reasoning, …) |

**Published by:** `brain-goals` GoalManager  
**Trigger:** Replan or recovery exhausted, or unrecoverable error  
**Consumers:** `brain-reflection` (reflect on failure), `brain-learning` (trigger mistake learning)

---

#### `brain.goal.cancelled`

| Field | Type | Description |
|---|---|---|
| `goal_id` | `GoalId` | |
| `reason` | `string` | Operator or policy reason |
| `cancelled_by` | `ActorId` | Who initiated the cancel |
| `cancelled_at_phase` | `BrainPhase?` | If mid-cycle, which phase was active |

**Published by:** `brain-goals` GoalManager  
**Trigger:** `cancel(goal_id)` — operator or policy  
**Consumers:** `brain-coordinator` (interrupt current tick)

---

#### `brain.goal.paused`

| Field | Type | Description |
|---|---|---|
| `goal_id` | `GoalId` | |
| `reason` | `string?` | Optional reason |

**Published by:** `brain-goals` GoalManager  
**Trigger:** Pause request (cooperative — only at phase boundaries)

---

#### `brain.goal.dependency.violation`

| Field | Type | Description |
|---|---|---|
| `goal_id` | `GoalId` | Goal that could not be activated |
| `blocking_dependency` | `GoalId` | The unsatisfied dependency |
| `violation_type` | `string` | `unresolved` / `cycle` / `not_found` |

**Published by:** `brain-goals` GoalValidator  
**Consumers:** `brain-coordinator` (emit diagnostic, do not enqueue)

---

### 2. Planner Events

#### `brain.plan.created`

| Field | Type | Description |
|---|---|---|
| `plan_id` | `PlanId` | |
| `goal_id` | `GoalId` | Parent goal |
| `step_count` | `usize` | Number of steps in the plan |
| `tool_requirements` | `Vec<ToolRequirementId>` | Abstract requirements (not OS commands) |
| `estimated_duration_ms` | `u64` | |
| `estimated_cost_cents` | `u64` | |
| `strategy_used` | `PlanningStrategy` | MeansEnds / ForwardChaining / BackwardChaining / TemplateMatch |

**Published by:** `brain-planner` Planner  
**Trigger:** `decompose(goal, budget)` returns an `ExecutablePlan`

---

#### `brain.plan.updated`

| Field | Type | Description |
|---|---|---|
| `plan_id` | `PlanId` | |
| `changes` | `string` | Summary of what changed |
| `reason` | `string` | Why it changed (replan, degradation, …) |

**Published by:** `brain-planner` Planner  
**Trigger:** `PlanOptimizer::optimize()`, or replan after policy rejection

---

#### `brain.plan.rejected`

| Field | Type | Description |
|---|---|---|
| `plan_id` | `PlanId` | |
| `goal_id` | `GoalId` | |
| `violated_rules` | `Vec<ViolatedRule>` | Policy rules that blocked the plan |

**Published by:** `brain-policy` PolicyEngine  
**Consumers:** `brain-coordinator` (triggers Recovering state or replan)

---

### 3. Reasoning Events

#### `brain.reasoning.started`

| Field | Type | Description |
|---|---|---|
| `thought_id` | `ThoughtId` | Unique reasoning session ID |
| `goal_id` | `GoalId` | Goal being reasoned about |
| `reasoning_depth` | `usize` | Starting depth |
| `hypotheses_requested` | `usize` | Max branches requested |

---

#### `brain.reasoning.finished`

| Field | Type | Description |
|---|---|---|
| `thought_id` | `ThoughtId` | |
| `selected_hypothesis_id` | `HypothesisId?` | None if budget exhausted |
| `confidence` | `Confidence` | Confidence in the selected hypothesis |
| `depth_reached` | `usize` | Actual depth used |
| `model_calls_used` | `usize` | |
| `tokens_consumed` | `u32` | |
| `budget_remaining` | `BudgetUsage` | Residual budget |

---

#### `brain.reasoning.hypothesis.generated`

| Field | Type | Description |
|---|---|---|
| `thought_id` | `ThoughtId` | Parent reasoning session |
| `hypothesis_id` | `HypothesisId` | |
| `confidence` | `Confidence` | Initial confidence score |

**Published by:** `brain-reasoner` HypothesisGenerator

---

#### `brain.budget.exhausted`

| Field | Type | Description |
|---|---|---|
| `budget_dimension` | `BudgetDimension` | Which resource ran out |
| `phase` | `BrainPhase` | Phase in which exhaustion occurred |
| `usage_at_exhaustion` | `BudgetUsage` | Current usage snapshot |
| `budget_snapshot` | `CognitiveBudget` | Budget that was exhausted |

**Published by:** `brain-coordinator`  
**Consumers:** `brain-planner` (degrade), `brain-reasoner` (return best effort), all phases

---

### 4. Decision Events

#### `brain.decision.made`

| Field | Type | Description |
|---|---|---|
| `decision_id` | `DecisionId` | |
| `plan_id` | `PlanId` | Plan that was decided on |
| `selected_option` | `DecisionOptionRef` | What was chosen |
| `confidence` | `Confidence` | |
| `candidates_considered` | `usize` | |
| `rationale_summary` | `string` | Short human-readable rationale |
| `policy_evaluation_passed` | `bool` | |

**Published by:** `brain-decision` DecisionMaker  
**Consumers:** `brain-workflow` (execute), `brain-reflection` (trigger later)

---

#### `brain.decision.rejected`

| Field | Type | Description |
|---|---|---|
| `plan_id` | `PlanId` | |
| `goal_id` | `GoalId` | |
| `violated_rules` | `Vec<ViolatedRule>` | Blocking policy violations |
| `policy_evaluation` | `PolicyEvaluation` | Full evaluation result |

**Published by:** `brain-policy` PolicyEngine  
**Consumers:** `brain-coordinator` (replan or escalate)

---

#### `brain.decision.escalation.required`

| Field | Type | Description |
|---|---|---|
| `decision_id` | `DecisionId?` | If a decision was partially formed |
| `conflict` | `ConflictRef` | What could not be resolved |
| `options` | `Vec<DecisionOptionRef>` | Competing options |
| `recommended_actor` | `EscalationChannel` | Operator / Human / Admin / Webhook |

**Published by:** `brain-decision` ConflictResolver  
**Consumers:** `brain-coordinator` (emit to operator channel)

---

### 5. Reflection Events

#### `brain.reflection.completed`

| Field | Type | Description |
|---|---|---|
| `reflection_id` | `ReflectionId` | |
| `decision_id` | `DecisionId` | Decision being reflected on |
| `match_score` | `f64` | 0.0–1.0 expected vs actual similarity |
| `mistakes_detected` | `Vec<MistakeId>` | |
| `improvements_generated` | `Vec<ImprovementId>` | |
| `lesson_stored` | `LessonId?` | If a lesson was persisted |

**Published by:** `brain-reflection` Reflector  
**Consumers:** `brain-learning` (trigger learning cycle)

---

#### `brain.reflection.outcome.observed`

| Field | Type | Description |
|---|---|---|
| `decision_id` | `DecisionId` | |
| `expected_outcome` | `string` | What the decision predicted |
| `actual_outcome` | `string` | What actually happened |
| `match_score` | `f64` | |

**Published by:** Observer (Phase 9) or Execution stub  
**Consumers:** `brain-reflection` Reflector (input to `reflect()`)

---

### 6. Learning Events

#### `brain.learning.cycle.started`

| Field | Type | Description |
|---|---|---|
| `source` | `LearningSource` | ReflectionCompleted / Mistake / Periodic / PerformanceDegraded |
| `trigger_event_id` | `EventId?` | Originating event if applicable |
| `scope` | `LearningScope` | Goal-level / Session-level / Global |

---

#### `brain.learning.cycle.completed`

| Field | Type | Description |
|---|---|---|
| `source` | `LearningSource` | |
| `promoted_count` | `u32` | Episodic memories promoted to semantic |
| `planner_adjustments` | `PlanningAdjustments?` | If any were made |
| `reasoner_heuristic_updates` | `u32` | Number of heuristic weights adjusted |

---

#### `brain.learning.promotion.applied`

| Field | Type | Description |
|---|---|---|
| `episodic_ids` | `Vec<Uuid>` | Source memory references |
| `target_tier` | `MemoryTier` | Semantic / Procedural / Generalised |
| `promotion_reason` | `PromotionReason` | RecurrenceCount / Importance / Recency |

---

### 7. Workflow Events

#### `brain.workflow.started`

| Field | Type | Description |
|---|---|---|
| `workflow_id` | `WorkflowId` | |
| `name` | `string` | Human-readable name |
| `step_count` | `usize` | |
| `composition` | `CompositionType` | Sequential / Parallel / Conditional / FanOut / FanIn |

---

#### `brain.workflow.step.started`

| Field | Type | Description |
|---|---|---|
| `workflow_id` | `WorkflowId` | |
| `step_id` | `Uuid` | |
| `step_name` | `string` | |

---

#### `brain.workflow.step.completed`

| Field | Type | Description |
|---|---|---|
| `workflow_id` | `WorkflowId` | |
| `step_id` | `Uuid` | |
| `outcome` | `StepOutcome` | Success / Failed / Skipped |
| `duration_ms` | `u64` | |

---

#### `brain.workflow.checkpointed`

| Field | Type | Description |
|---|---|---|
| `workflow_id` | `WorkflowId` | |
| `checkpoint_id` | `CheckpointId` | |
| `step_index` | `usize` | Steps completed so far |
| `total_steps` | `usize` | |
| `size_bytes` | `u32` | Serialized checkpoint size |

**Published by:** `brain-workflow` CheckpointManager

---

#### `brain.workflow.completed`

| Field | Type | Description |
|---|---|---|
| `workflow_id` | `WorkflowId` | |
| `total_duration_ms` | `u64` | |
| `steps_completed` | `usize` | |
| `steps_failed` | `usize` | |

---

#### `brain.workflow.failed`

| Field | Type | Description |
|---|---|---|
| `workflow_id` | `WorkflowId` | |
| `failed_step_id` | `Uuid?` | |
| `error` | `BrainError` | |
| `recoverable` | `bool` | Whether recovery was attempted |

---

### 8. Coordinator / Lifecycle Events

#### `brain.started`

| Field | Type | Description |
|---|---|---|
| `version` | `string` | Brain Platform version |
| `subsystems_loaded` | `Vec<string>` | Names of successfully started crates |
| `active_sessions` | `u32` | |

**Published by:** `brain-coordinator`  
**Trigger:** `BrainPlatform::start()` completes

---

#### `brain.paused`

| Field | Type | Description |
|---|---|---|
| `reason` | `string?` | Operator or policy |
| `active_goals_preserved` | `u32` | Goals left in-flight |

**Published by:** `brain-coordinator`  
**Consumers:** All Brain crates (suspend processing)

---

#### `brain.resumed`

| Field | Type | Description |
|---|---|---|
| `previous_state` | `BrainState` | State before pause |

---

#### `brain.stopping`

| Field | Type | Description |
|---|---|---|
| `reason` | `string` | Normal shutdown / Error / Operator |

**Published by:** `brain-coordinator`  
**Trigger:** `stop()` invoked; precedes the stop sequence

---

#### `brain.stopped`

| Field | Type | Description |
|---|---|---|
| `uptime_ms` | `u64` | |
| `goals_completed` | `u32` | |
| `goals_failed` | `u32` | |

---

#### `brain.failed`

| Field | Type | Description |
|---|---|---|
| `error` | `BrainError` | Terminal error |
| `recovery_attempts` | `u32` | |
| `state_at_failure` | `BrainState` | |

**Published by:** `brain-coordinator`  
**Consumers:** Operator / monitoring systems

---

### 9. Policy Events

#### `brain.policy.evaluated`

| Field | Type | Description |
|---|---|---|
| `plan_id` | `PlanId` | |
| `evaluation_id` | `EvaluationId` | |
| `passed` | `bool` | |
| `rules_evaluated` | `u32` | |
| `violations` | `Vec<ViolatedRule>` | |
| `duration_us` | `u64` | |

**Published by:** `brain-policy` PolicyEngine  
**Consumers:** `brain-decision` DecisionMaker (input to `decide`)

---

#### `brain.policy.reloaded`

| Field | Type | Description |
|---|---|---|
| `source` | `string` | `config_file` / `hot_reload` / `api` |
| `rulesets_loaded` | `u32` | |
| `guard_rails_loaded` | `u32` | |

**Published by:** `brain-policy` PolicyLoader  
**Consumers:** `brain-policy` PolicyEngine (refresh in-memory rules)

---

## Consumed Events

Events published by other layers that Brain subscribes to.

| Event | Source Layer | Consumer(s) | Action |
|---|---|---|---|
| `memory.item.retrieved` | Memory (4) | Reasoner, GoalManager | Enrich reasoning context and goal state |
| `memory.consolidation.completed` | Memory (4) | GoalManager | Re-evaluate pending goals |
| `memory.item.promoted` | Memory (4) | Learner | Acknowledge promotion; update heuristics |
| `context.session.started` | Core / Runtime | Coordinator | Bind new session; initialise context |
| `context.session.ended` | Core / Runtime | Coordinator | Tear down session-scoped state |
| `task.completed` | Runtime (3) | WorkflowEngine, Reflector | Advance step; capture outcome |
| `task.failed` | Runtime (3) | WorkflowEngine, Reflector | Trigger recovery flow |
| `execution.result.available` | Execution (8, future) | Reflector | Actual outcome for comparison |
| `perception.observation.new` | Perception (7, future) | Reasoner | New sensor input for reasoning |
| `perception.world_changed` | Perception (7, future) | Coordinator | Re-evaluate active goals |
| `model.provider.available` | `brain-model` | Coordinator, Reasoner | Update provider registry |
| `model.provider.unavailable` | `brain-model` | Coordinator | Emit `DegradedMode`; use cached reasoning |
| `policy.rule.changed` | `brain-policy` | PolicyEngine | Hot-reload guard rails and rules |
| `operator.command` | External / Operator UI | Coordinator | Pause / Resume / Cancel / Override |

---

## Event Traceability

Every Brain event carries:

- `trace_id` — end-to-end request correlation
- `span_id` — within-trace segment
- `session_id` — operator or user session
- `goal_id?` — if the event is goal-scoped
- `timestamp` — `CognitiveTimestamp` from Core's clock

This enables full causal replay: any decision can be traced back from `decision_made` → `plan_created` → `goal_activated` → `goal_created`.

---

## Cross-References

- [`docs/architecture/brain.md`](brain.md) — overall crate structure and dependency rules
- [`docs/cognitive-loop.md`](cognitive-loop.md) — state machines and flow diagrams
- [`docs/architecture/brain-dependencies.md`](brain-dependencies.md) — inter-crate communication matrix
- [`docs/rfc/RFC-0003-brain-platform.md`](../rfc/RFC-0003-brain-platform.md) — original approved design
