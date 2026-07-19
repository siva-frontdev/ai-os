# Brain Platform Module

**Module:** `ai_os_brain`
**Status:** Planned (Phase 6)
**Crate:** Not yet created

## Purpose

The Brain Platform module implements the core cognitive loop of the AI-native OS. It plans, executes, and evaluates sequences of actions toward goals; makes utility- and rule-based decisions; manages a hierarchy of goals with dependency tracking; recognizes user intent from natural language or structured input; and maintains a knowledge graph of entities, relations, and facts. The Brain is the highest-level reasoning coordinator -- it does not execute actions directly but delegates to lower modules.

## Status Overview

| Subsystem | Design | Implementation | Tests |
|---|---|---|---|
| ReasoningEngine | Draft | Not started | Not started |
| DecisionMaker | Draft | Not started | Not started |
| GoalManager | Draft | Not started | Not started |
| IntentRecognizer | Draft | Not started | Not started |
| KnowledgeGraph | Draft | Not started | Not started |

## Public Interfaces

### `ReasoningEngine`

Planned struct implementing the planner-executor-evaluator loop using means-end analysis.

```rust
/// Planned for Phase 6
pub struct ReasoningEngine {
    planner: Arc<dyn Planner>,
    executor: Arc<dyn Executor>,
    evaluator: Arc<dyn Evaluator>,
    context: ContextManager,
}

/// Planned for Phase 6
#[async_trait]
pub trait Planner: Send + Sync {
    /// Given a goal and current state, produce an ordered plan.
    async fn plan(&self, goal: &Goal, state: &SystemState) -> Result<Plan, BrainError>;
}

/// Planned for Phase 6
#[async_trait]
pub trait Executor: Send + Sync {
    /// Execute a single step. Returns observation.
    async fn execute(&self, step: &Step) -> Result<Observation, BrainError>;
}

/// Planned for Phase 6
#[async_trait]
pub trait Evaluator: Send + Sync {
    /// Evaluate whether the observation moves the system toward the goal.
    fn evaluate(&self, observation: &Observation, expected: &Outcome) -> Evaluation;
}

impl ReasoningEngine {
    pub fn new(planner: Arc<dyn Planner>, executor: Arc<dyn Executor>, evaluator: Arc<dyn Evaluator>) -> Self;

    /// Run the cognitive loop: plan -> execute steps -> evaluate -> re-plan if needed.
    pub async fn reason(&self, goal: &Goal) -> Result<Outcome, BrainError>;
}
```

The loop:
1. `planner.plan(goal, state)` produces a `Vec<Step>`.
2. For each step, `executor.execute(step)` returns an `Observation`.
3. `evaluator.evaluate(observation, expected)` returns `OnTrack`, `OffTrack(correction)`, or `Blocked(obstacle)`.
4. If `OffTrack` or `Blocked`, the engine re-plans from the current state.

### `DecisionMaker`

Planned struct combining utility-based and rule-based decision logic.

```rust
/// Planned for Phase 6
pub struct DecisionMaker {
    utility_functions: Vec<Arc<dyn UtilityFunction>>,
    rules: Vec<Arc<dyn DecisionRule>>,
}

/// Planned for Phase 6
#[async_trait]
pub trait UtilityFunction: Send + Sync {
    fn name(&self) -> &str;
    async fn evaluate(&self, options: &[Action]) -> Vec<f64>;
}

/// Planned for Phase 6
#[async_trait]
pub trait DecisionRule: Send + Sync {
    fn name(&self) -> &str;
    async fn applies(&self, context: &DecisionContext) -> bool;
    async fn rank(&self, options: &[Action]) -> Vec<Action>;
}

impl DecisionMaker {
    pub fn new() -> Self;

    /// Select the best action. Applies rules first (hard constraints),
    /// then ranks remaining options by weighted utility.
    pub async fn decide(&self, context: &DecisionContext, options: &[Action]) -> Result<Action, BrainError>;

    /// Cost-benefit analysis comparing two action sets.
    pub async fn cost_benefit(&self, a: &[Action], b: &[Action]) -> Result<CBAReport, BrainError>;
}

/// Planned for Phase 6
pub struct CBAReport {
    pub expected_cost: f64,
    pub expected_benefit: f64,
    pub confidence: f64,
    pub breakdown: HashMap<String, f64>,
}
```

### `GoalManager`

Planned struct for creating, tracking, and decomposing goals.

```rust
/// Planned for Phase 6
pub struct GoalManager {
    goals: RwLock<HashMap<GoalId, Goal>>,
    dependencies: RwLock<HashMap<GoalId, Vec<GoalId>>>,
}

/// Planned for Phase 6
pub struct Goal {
    pub id: GoalId,
    pub description: String,
    pub status: GoalStatus,
    pub priority: Priority,
    pub parent: Option<GoalId>,
    pub subgoals: Vec<GoalId>,
    pub progress: f64,           // 0.0 .. 1.0
    pub created_at: SystemTime,
    pub deadline: Option<SystemTime>,
}

/// Planned for Phase 6
pub enum GoalStatus {
    Active,
    Paused,
    Completed,
    Failed,
    Cancelled,
}

impl GoalManager {
    pub fn new() -> Self;

    pub fn create(&mut self, description: &str, priority: Priority) -> GoalId;
    pub fn get(&self, id: &GoalId) -> Option<Goal>;
    pub fn update(&mut self, id: &GoalId, updates: GoalUpdate) -> Result<(), BrainError>;
    pub fn delete(&mut self, id: &GoalId) -> Result<(), BrainError>;

    /// Decompose a goal into subgoals.
    pub fn decompose(&mut self, parent: &GoalId, subgoals: Vec<Goal>) -> Result<(), BrainError>;

    /// Return goals that must be completed before the given goal can start.
    pub fn dependencies(&self, id: &GoalId) -> Vec<GoalId>;

    /// Topological sort of goal graph.
    pub fn sorted(&self) -> Result<Vec<Goal>, BrainError>;
}
```

### `IntentRecognizer`

Planned struct for mapping input to structured intent.

```rust
/// Planned for Phase 6
pub struct IntentRecognizer {
    patterns: Vec<IntentPattern>,
    nlu_backend: Option<Arc<dyn NluEngine>>,
}

/// Planned for Phase 6
pub struct IntentPattern {
    pub name: String,
    pub confidence_threshold: f64,
    pub matcher: Box<dyn PatternMatcher>,
}

/// Planned for Phase 6
pub struct Intent {
    pub action: String,              // e.g. "schedule_task"
    pub confidence: f64,
    pub slots: HashMap<String, String>,
    pub raw_input: String,
}

/// Planned for Phase 6
#[async_trait]
pub trait PatternMatcher: Send + Sync {
    async fn matches(&self, input: &str) -> Option<f64>;
}

/// Planned for Phase 6
#[async_trait]
pub trait NluEngine: Send + Sync {
    async fn classify(&self, input: &str) -> Result<Vec<Intent>, BrainError>;
    fn is_available(&self) -> bool;
}

impl IntentRecognizer {
    pub fn new(patterns: Vec<IntentPattern>) -> Self;

    /// Register a pattern-based matcher.
    pub fn register_pattern(&mut self, pattern: IntentPattern);

    /// Attach an NLU engine (optional, used when pattern confidence is low).
    pub fn attach_nlu(&mut self, engine: Arc<dyn NluEngine>);

    /// Recognize intent. Tries patterns first; falls back to NLU if below threshold.
    pub async fn recognize(&self, input: &str) -> Result<Intent, BrainError>;
}
```

### `KnowledgeGraph`

Planned struct for storing entities, binary relations, and facts.

```rust
/// Planned for Phase 6
pub struct KnowledgeGraph {
    entities: RwLock<HashMap<EntityId, Entity>>,
    relations: RwLock<Vec<Relation>>,
    facts: RwLock<Vec<Fact>>,
}

/// Planned for Phase 6
pub struct Entity {
    pub id: EntityId,
    pub name: String,
    pub kind: String,
    pub properties: HashMap<String, String>,
}

/// Planned for Phase 6
pub struct Relation {
    pub id: RelationId,
    pub subject: EntityId,
    pub predicate: String,
    pub object: EntityId,
    pub weight: f64,
}

/// Planned for Phase 6
pub struct Fact {
    pub id: FactId,
    pub statement: String,
    pub confidence: f64,
    pub source: String,
    pub timestamp: SystemTime,
}

impl KnowledgeGraph {
    pub fn new() -> Self;

    pub fn add_entity(&mut self, entity: Entity) -> EntityId;
    pub fn add_relation(&mut self, relation: Relation) -> RelationId;
    pub fn add_fact(&mut self, fact: Fact) -> FactId;

    pub fn get_entity(&self, id: &EntityId) -> Option<Entity>;
    pub fn query_relations(&self, subject: &EntityId, predicate: &str) -> Vec<Relation>;

    /// Traverse the graph from a starting entity along edges matching a predicate.
    pub fn traverse(&self, start: &EntityId, predicate: &str, depth: usize) -> Vec<Entity>;

    /// Consolidate duplicate entities by name and kind.
    pub fn deduplicate(&mut self);
}
```

## Dependencies

| Dependency | Scope | Purpose |
|---|---|---|
| `ai_os_core` | internal | EventBus, Service, Logger, Container |
| `ai_os_memory` | internal | Knowledge persistence via LongTermStorage |
| `ai_os_runtime` | internal | Task scheduling for plan steps |
| `serde` | external | Serialization of goals, entities, facts |
| `petgraph` | external | Directed graph for goal dependencies and knowledge graph |
| `tokio` | runtime | Async cognitive loop |

Intents may use `regex` for pattern matching; the NLU interface is designed to wrap an external service (e.g., a local LLM via REST or gRPC).

## Events Published

| Event | Trigger | Payload |
|---|---|---|
| `brain::GoalCreated` | `GoalManager::create` | `(GoalId, String)` -- id, description |
| `brain::GoalCompleted` | Goal reaches 100% progress | `GoalId` |
| `brain::GoalFailed` | Goal status set to Failed | `(GoalId, String)` -- reason |
| `brain::PlanProduced` | `ReasoningEngine` generates plan | `(GoalId, usize)` -- goal id, step count |
| `brain::DecisionMade` | `DecisionMaker::decide` | `(DecisionContext, Action)` |
| `brain::IntentRecognized` | `IntentRecognizer::recognize` | `Intent` |

## Events Consumed

| Event | Consumer | Action |
|---|---|---|
| `perception::InputEvent` | `IntentRecognizer` | Process incoming user input |
| `runtime::TaskCompleted` | `ReasoningEngine` | Feed result as observation |
| `memory::ItemPromoted` | `KnowledgeGraph` | Extract entities from promoted text |
| `system::ProcessExited` | `Evaluator` | Evaluate process outcome against expected |

## Thread Model

- `GoalManager` and `KnowledgeGraph` are read-heavy structures behind `RwLock`. Reads (traversal, sorted, get) acquire the read lock; writes (create, update, add) acquire the write lock.
- `DecisionMaker` utility functions are `Send + Sync` and evaluated in parallel with `tokio::join!` or `FuturesUnordered`.
- `ReasoningEngine` runs on a single Tokio task per reasoning session; multiple sessions may run concurrently, each with its own state.
- `IntentRecognizer` is stateless (patterns are immutable after registration) and `Send + Sync`.

## Lifecycle

```
Boot sequence:
  1. KnowledgeGraph::new (empty graph)
  2. GoalManager::new (load goals from LTM if resuming)
  3. IntentRecognizer::new (register built-in patterns)
  4. DecisionMaker::new (register utility functions and rules)
  5. ReasoningEngine::new (wire components together)
```

The Brain does not have a `run` loop; it is driven by events. The `IntentRecognizer` listens on the EventBus for `InputEvent` and calls `recognize`. The resulting `Intent` is routed to `GoalManager` (create a goal) or `ReasoningEngine` (execute a plan).

Shutdown persists all active goals to LongTermStorage and flushes pending decisions.

## Error Handling

Error type: `ai_os_brain::BrainError` with variants:

- `PlanningError(String)` -- planner cannot find a valid plan.
- `ExecutionError(String)` -- step execution failed irrecoverably.
- `EvaluationError(String)` -- evaluator could not determine outcome.
- `GoalCycleError(String)` -- goal dependency graph has a cycle.
- `IntentNotRecognized(String)` -- input confidence below threshold.
- `GraphConsistencyError(String)` -- dangling entity or relation reference.

Recovery: `ReasoningEngine` retries failed steps up to 3 times. If `IntentNotRecognized` is returned, the system publishes a `ClarificationRequest` event and waits for follow-up input. `GoalCycleError` is a programmer error caught at integration test time.

## Configuration

```toml
[reasoning]
max_plan_steps = 50
retry_limit = 3
replan_threshold = 0.6

[decision]
default_utility = "maximin"

[intent]
pattern_confidence_threshold = 0.7
fallback_to_nlu = true

[goals]
max_active = 20
enable_persistence = true
```

Configuration file: `aios-brain.toml`.

## Testing Strategy

- **Unit tests**: GoalManager CRUD, dependency graph topological sort, cycle detection. KnowledgeGraph add/query/traverse. DecisionMaker utility ranking against known options. IntentRecognizer pattern matching with golden test cases.
- **Integration tests**: ReasoningEngine end-to-end loop with mock planner/executor/evaluator. Verify re-plan triggers on `OffTrack`. Goal decomposition and subgoal progress propagation.
- **Property-based tests**: KnowledgeGraph deduplication is idempotent. Goal dependency graph sorting returns a valid topological order for any acyclic graph.
- **Scenario tests**: Full cognitive loop against a simulated environment -- system receives intent, creates goal, produces plan, executes steps, evaluates, completes.

## Future Extensions

- Meta-cognition: the Brain monitors its own reasoning performance and adjusts parameters (e.g., switching planner strategy when plans repeatedly fail).
- Multi-agent reasoning: multiple `ReasoningEngine` instances coordinate via shared goals in the `GoalManager`.
- Explanation generation: produce natural-language explanations for decisions (useful for audit and user trust).
- Counterfactual reasoning: simulate alternative action sequences offline.
- Integration with an external LLM reasoning service via the NLU interface.
- Episodic memory replay: use consolidated memories from `Memory` to inform planning (analogous to hippocampal replay).
