# Brain Platform Blueprint

## Purpose

The Brain Platform (Phase 6, Planned) is the reasoning and decision-making layer of AI-native OS. It interprets events from Perception, retrieves knowledge from Memory, decomposes high-level goals into executable tasks, and orchestrates their execution through the Runtime Platform. The Brain is not an LLM integration layer (that belongs to Phase 9, Intelligence Integration); it is the structured reasoning engine that operates on explicit knowledge representations -- plans, policies, goals, and a knowledge graph -- to drive autonomous platform behavior.

The Brain Platform builds on the [Memory Platform](memory.md) (knowledge retrieval), [Perception Platform](perception.md) (event interpretation), and [Runtime Platform](runtime.md) (task execution). It depends on no other AI-native OS crate beyond Core, Runtime, Memory, and Perception.

---

## Responsibilities

- **Reasoning Engine**: Implement a cyclical planner-executor-evaluator loop. Accept a goal, generate a plan (sequence of actions), execute each action via the Runtime Platform, evaluate the outcome, and replan on failure or unexpected state. Support forward chaining (goal-driven) and backward chaining (evidence-driven) reasoning strategies.
- **Decision Making**: Evaluate choices using policy-based rules (if-then conditions with configurable priorities) and utility-based evaluation (score each candidate action against a utility function). Support multi-attribute utility functions with weighted dimensions (cost, time, risk, resource usage).
- **Goal Management**: Maintain a goal tree where high-level goals decompose into subgoals. Track goal state (Active, InProgress, Suspended, Completed, Failed, Abandoned). Support goal dependencies, prerequisites, and conflicts. Provide goal lifecycle events for observability.
- **Intent Recognition**: Accept high-level natural language or structured intent descriptors and map them to known goal templates. Use pattern matching against a registry of intent templates. Each template defines parameter extraction rules and goal generation logic.
- **Knowledge Graph**: Maintain an in-memory entity-relation store with typed entities (concept, resource, service, file, session), typed relations (depends_on, owns, contains, produces, requires), and facts (entity-relation-entity triples with confidence scores). Support SPARQL-like graph queries and subgraph matching.
- **Chain-of-Thought Orchestration**: Manage multi-step reasoning traces. Each step records the action taken, inputs consumed, output produced, and confidence score. Support branching (parallel exploration of alternatives) and backtracking (abandon a branch and restore prior state).

---

## Public Interfaces

### Reasoner (`brain::reason`)

```rust
pub enum ReasoningStrategy {
    ForwardChaining,
    BackwardChaining,
    MeansEndsAnalysis,
}

pub struct Plan {
    pub id: Uuid,
    pub goal_id: Uuid,
    pub steps: Vec<PlanStep>,
    pub strategy: ReasoningStrategy,
    pub created_at: DateTime<Utc>,
    pub status: PlanStatus,
}

pub struct PlanStep {
    pub id: Uuid,
    pub action: String,
    pub parameters: HashMap<String, String>,
    pub dependencies: Vec<Uuid>,
    pub expected_outcome: String,
    pub status: StepStatus,
}

pub enum PlanStatus { Pending, InProgress, Completed, Failed, Replanned }
pub enum StepStatus { Pending, Ready, Running, Succeeded, Failed, Skipped }

pub trait Reasoner: Debug + Send + Sync {
    async fn create_plan(&self, goal_id: Uuid, strategy: ReasoningStrategy) -> Result<Plan, BrainError>;
    async fn execute_step(&self, step_id: Uuid) -> Result<StepOutcome, BrainError>;
    async fn evaluate_step(&self, step_id: Uuid, outcome: &StepOutcome) -> Result<Evaluation, BrainError>;
    async fn replan(&self, plan_id: Uuid, failed_step_id: Uuid, reason: &str) -> Result<Plan, BrainError>;
    async fn get_plan(&self, plan_id: Uuid) -> Result<Plan, BrainError>;
    fn active_plans(&self) -> Vec<Plan>;
}
```

### DecisionMaker (`brain::decision`)

```rust
pub struct DecisionRequest {
    pub context: HashMap<String, f64>,
    pub candidates: Vec<CandidateAction>,
    pub policy_id: Option<Uuid>,
    pub utility_weights: Option<UtilityWeights>,
}

pub struct CandidateAction {
    pub id: Uuid,
    pub name: String,
    pub estimated_cost: f64,
    pub estimated_duration: Duration,
    pub estimated_risk: f64,
    pub resource_demand: ResourceUsage,
}

pub struct UtilityWeights {
    pub cost: f64,
    pub duration: f64,
    pub risk: f64,
    pub resource_efficiency: f64,
}

pub enum DecisionStrategy { PolicyFirst, UtilityMaximizing, Satisficing }

pub trait DecisionMaker: Debug + Send + Sync {
    async fn evaluate(&self, request: DecisionRequest) -> Result<Decision, BrainError>;
    async fn evaluate_policy(&self, request: &DecisionRequest) -> Result<Option<Decision>, BrainError>;
    async fn evaluate_utility(&self, request: &DecisionRequest) -> Result<Decision, BrainError>;
}
```

### GoalManager (`brain::goal`)

```rust
pub struct Goal {
    pub id: Uuid,
    pub parent_id: Option<Uuid>,
    pub name: String,
    pub description: String,
    pub state: GoalState,
    pub priority: u8,
    pub created_at: DateTime<Utc>,
    pub deadline: Option<DateTime<Utc>>,
    pub dependencies: Vec<Uuid>,
    pub subgoals: Vec<Goal>,
    pub result: Option<GoalResult>,
}

pub enum GoalState { Active, InProgress, Suspended, Completed, Failed, Abandoned }

pub trait GoalManager: Debug + Send + Sync {
    async fn create_goal(&self, name: &str, description: &str) -> Result<Goal, BrainError>;
    async fn decompose_goal(&self, parent_id: Uuid, subgoals: Vec<GoalSpec>) -> Result<Vec<Goal>, BrainError>;
    async fn update_state(&self, goal_id: Uuid, state: GoalState) -> Result<(), BrainError>;
    async fn get_goal(&self, goal_id: Uuid) -> Result<Goal, BrainError>;
    async fn get_goal_tree(&self, root_id: Uuid) -> Result<Goal, BrainError>;
    async fn find_conflicts(&self, goal_ids: &[Uuid]) -> Result<Vec<Conflict>, BrainError>;
    fn list_active_goals(&self) -> Vec<Goal>;
}
```

### IntentRecognizer (`brain::intent`)

```rust
pub struct IntentTemplate {
    pub id: Uuid,
    pub name: String,
    pub patterns: Vec<String>,
    pub parameters: Vec<ParameterSchema>,
    pub goal_template: String,
}

pub struct ParameterSchema {
    pub name: String,
    pub required: bool,
    pub param_type: ParamType,
    pub default: Option<String>,
}

pub enum ParamType { String, Number, Duration, EntityRef, FilePath }

pub trait IntentRecognizer: Debug + Send + Sync {
    async fn register_template(&self, template: IntentTemplate) -> Result<(), BrainError>;
    async fn recognize(&self, input: &str) -> Result<Option<RecognizedIntent>, BrainError>;
    async fn extract_parameters(&self, input: &str, template_id: Uuid) -> Result<HashMap<String, String>, BrainError>;
    fn list_templates(&self) -> Vec<IntentTemplate>;
}
```

### KnowledgeGraph (`brain::knowledge`)

```rust
pub struct Entity {
    pub id: Uuid,
    pub entity_type: String,
    pub name: String,
    pub properties: HashMap<String, String>,
}

pub struct Relation {
    pub id: Uuid,
    pub source_id: Uuid,
    pub target_id: Uuid,
    pub relation_type: String,
    pub weight: f64,
    pub confidence: f64,
}

pub struct Fact {
    pub subject_id: Uuid,
    pub predicate: String,
    pub object_id: Uuid,
    pub confidence: f64,
    pub source: String,
}

pub trait KnowledgeGraph: Debug + Send + Sync {
    async fn add_entity(&self, entity: Entity) -> Result<(), BrainError>;
    async fn add_relation(&self, relation: Relation) -> Result<(), BrainError>;
    async fn add_fact(&self, fact: Fact) -> Result<(), BrainError>;
    async fn get_entity(&self, entity_id: Uuid) -> Result<Entity, BrainError>;
    async fn find_entities(&self, entity_type: &str, name: &str) -> Result<Vec<Entity>, BrainError>;
    async fn query_neighbors(&self, entity_id: Uuid, max_depth: u8) -> Result<Subgraph, BrainError>;
    async fn query_path(&self, from_id: Uuid, to_id: Uuid) -> Result<Vec<PathSegment>, BrainError>;
}
```

### ChainOfThought (`brain::chain`)

```rust
pub struct ThoughtChain {
    pub id: Uuid,
    pub plan_id: Uuid,
    pub steps: Vec<ThoughtStep>,
    pub branches: Vec<ThoughtBranch>,
    pub status: ChainStatus,
}

pub struct ThoughtStep {
    pub id: Uuid,
    pub step_index: u32,
    pub action: String,
    pub input_summary: String,
    pub output_summary: String,
    pub confidence: f64,
    pub duration: Duration,
}

pub struct ThoughtBranch {
    pub id: Uuid,
    pub parent_step_id: Uuid,
    pub hypothesis: String,
    pub steps: Vec<ThoughtStep>,
    pub selected: bool,
}

pub trait ChainOfThought: Debug + Send + Sync {
    async fn start_chain(&self, plan_id: Uuid) -> Result<ThoughtChain, BrainError>;
    async fn append_step(&self, chain_id: Uuid, step: ThoughtStep) -> Result<(), BrainError>;
    async fn fork_branch(&self, chain_id: Uuid, parent_step_id: Uuid, hypothesis: &str) -> Result<ThoughtBranch, BrainError>;
    async fn select_branch(&self, chain_id: Uuid, branch_id: Uuid) -> Result<(), BrainError>;
    async fn backtrack(&self, chain_id: Uuid, to_step_index: u32) -> Result<(), BrainError>;
    fn get_chain(&self, chain_id: Uuid) -> Result<ThoughtChain, BrainError>;
}
```

### BrainPlatform (`brain::BrainPlatform`)

```rust
pub struct BrainPlatform {
    pub reasoner: Arc<dyn Reasoner>,
    pub decision_maker: Arc<dyn DecisionMaker>,
    pub goal_manager: Arc<dyn GoalManager>,
    pub intent_recognizer: Arc<dyn IntentRecognizer>,
    pub knowledge_graph: Arc<dyn KnowledgeGraph>,
    pub chain_of_thought: Arc<dyn ChainOfThought>,
}
```

`BrainPlatform` implements the Core `Service` trait. Its `start()` method initializes the knowledge graph from Memory, loads intent templates, and spawns the goal monitoring task.

---

## Dependencies

| Crate | Purpose | Subsystem |
|---|---|---|
| `ai-os-core` | EventBus, Service, Logger, LifecycleManager | All subsystems |
| `ai-os-runtime` | Scheduler, TaskManager, Supervisor, PermissionChecker | Reasoner, DecisionMaker |
| `ai-os-memory` | MemoryRetriever, MemoryStore for knowledge persistence | KnowledgeGraph, GoalManager |
| `ai-os-perception` | Percept classification and context enrichment | IntentRecognizer |
| `serde` / `serde_json` | Goal, plan, and graph serialization | All subsystems |
| `petgraph` | Directed graph data structure for knowledge graph | KnowledgeGraph |
| `regex` | Pattern matching for intent recognition | IntentRecognizer |
| `chrono` | Timestamps for goal management and chain steps | GoalManager, ChainOfThought |
| `uuid` | Identifiers for plans, goals, entities | All subsystems |
| `thiserror` | Error type derivation | All subsystems |

---

## Events Published

| Event | Type String | Subsystem | Trigger |
|---|---|---|---|
| `PlanCreated` | `brain.plan.created` | Reasoner | New plan generated from goal |
| `PlanStepStarted` | `brain.plan.step_started` | Reasoner | Step execution begins |
| `PlanStepCompleted` | `brain.plan.step_completed` | Reasoner | Step execution succeeds |
| `PlanStepFailed` | `brain.plan.step_failed` | Reasoner | Step execution fails |
| `PlanReplanned` | `brain.plan.replanned` | Reasoner | Plan regenerated after failure |
| `PlanCompleted` | `brain.plan.completed` | Reasoner | All steps completed successfully |
| `DecisionMade` | `brain.decision.made` | DecisionMaker | Action selected from candidates |
| `GoalCreated` | `brain.goal.created` | GoalManager | New goal registered |
| `GoalDecomposed` | `brain.goal.decomposed` | GoalManager | Goal split into subgoals |
| `GoalStateChanged` | `brain.goal.state_changed` | GoalManager | Goal transitions to new state |
| `GoalCompleted` | `brain.goal.completed` | GoalManager | Goal reaches Completed state |
| `GoalFailed` | `brain.goal.failed` | GoalManager | Goal reaches Failed state |
| `IntentRecognized` | `brain.intent.recognized` | IntentRecognizer | Input matched to intent template |
| `KnowledgeEntityAdded` | `brain.knowledge.entity_added` | KnowledgeGraph | New entity inserted |
| `KnowledgeRelationAdded` | `brain.knowledge.relation_added` | KnowledgeGraph | New relation inserted |
| `KnowledgeFactAdded` | `brain.knowledge.fact_added` | KnowledgeGraph | New fact asserted |
| `ThoughtBranchForked` | `brain.chain.branch_forked` | ChainOfThought | Alternative reasoning branch created |
| `ThoughtBranchSelected` | `brain.chain.branch_selected` | ChainOfThought | Branch chosen for continuation |

---

## Events Consumed

| Event | Source | Consumer | Purpose |
|---|---|---|---|
| `memory.item.retrieved` | MemoryRetriever | KnowledgeGraph | Update entity access recency |
| `memory.consolidation.completed` | MemoryConsolidator | GoalManager | Re-evaluate goals affected by new knowledge |
| `perception.percept.classified` | Perception Platform | IntentRecognizer | Convert classified percept into intent |
| `perception.context.enriched` | Perception Platform | KnowledgeGraph | Extract entities from enriched context |
| `runtime.task_completed` | TaskManager | Reasoner | Advance plan step to completed |
| `runtime.task_failed` | TaskManager | Reasoner | Trigger step failure handling and replanning |
| `runtime.task_cancelled` | TaskManager | Reasoner | Cancel associated plan step |
| `system.resource.warning` | SystemResourceManager | DecisionMaker | Adjust utility weights under resource pressure |

The Brain is the primary consumer of execution outcomes from the Runtime Platform. Every `TaskCompleted` and `TaskFailed` event advances the planner-evaluator loop.

---

## Thread Model

| Subsystem | Threading |
|---|---|
| `Reasoner` | `RwLock<HashMap<Uuid, Plan>>` for plan storage. Plan computation on caller's task. Replanning uses `spawn_blocking` for complex backward chaining. |
| `DecisionMaker` | Stateless policy evaluator. Utility computation on caller's task. Policy rules loaded at startup into an `Arc<Vec<PolicyRule>>`. |
| `GoalManager` | `RwLock<HashMap<Uuid, Goal>>` for goal storage. Tree traversal on caller's task. Conflict detection uses `petgraph` on a read lock. |
| `IntentRecognizer` | `RwLock<HashMap<Uuid, IntentTemplate>>` for template registry. Regex matching on caller's task. Parameter extraction may spawn for complex parsing. |
| `KnowledgeGraph` | `RwLock<petgraph::StableGraph>` for entities and relations. Graph queries under read lock. Mutations under write lock. |
| `ChainOfThought` | `RwLock<HashMap<Uuid, ThoughtChain>>` for chain storage. All operations on caller's task. |

No dedicated background threads are required. Goal monitoring runs as a Tokio periodic task on the shared runtime.

---

## Lifecycle

`BrainPlatform` implements Core's `Service` trait:

1. **Construction**: `BrainPlatform::new()` creates all subsystem instances. The knowledge graph is initialized empty; its content is loaded from Memory during `start()`.
2. **Start**: `start()` loads the persisted knowledge graph from the [Memory Platform](memory.md), registers intent templates from configuration, seeds the goal tree with any pending goals from a prior session, subscribes to Runtime and Memory events, and spawns the goal monitoring periodic task.
3. **Running**: The Brain processes incoming events, executes the planner loop for active goals, evaluates decisions, and updates the knowledge graph. Goals are decomposed and executed as tasks through the Runtime Platform.
4. **Stop**: `stop()` persists the knowledge graph to long-term Memory, archives incomplete plans and their chain-of-thought traces, unsubscribes from all events, and cancels the goal monitoring task.

Start order within the Brain Platform:

```
1. KnowledgeGraph     (foundation for all reasoning)
2. GoalManager        (goal tree must be ready)
3. IntentRecognizer   (loads templates)
4. Reasoner           (depends on goals and knowledge)
5. DecisionMaker      (depends on policies loaded from config)
6. ChainOfThought     (depends on plan execution)
7. Event subscriptions registered last
```

---

## Error Handling

`BrainError` is the unified error type:

| Variant | Subsystem | Condition |
|---|---|---|
| `PlanCreationFailed(String)` | Reasoner | Goal has no valid decomposition path |
| `StepExecutionFailed(Uuid, String)` | Reasoner | Runtime task returned error |
| `ReplanFailed(Uuid, String)` | Reasoner | No valid alternative plan exists |
| `DecisionUndecidable(String)` | DecisionMaker | All candidates filtered by policy, none remain |
| `GoalNotFound(Uuid)` | GoalManager | Referenced goal does not exist |
| `GoalCycleDetected(Vec<Uuid>)` | GoalManager | Dependency graph contains a cycle |
| `IntentNotRecognized(String)` | IntentRecognizer | Input does not match any template |
| `IntentAmbiguous(String, Vec<Uuid>)` | IntentRecognizer | Input matches multiple templates |
| `EntityNotFound(Uuid)` | KnowledgeGraph | Referenced entity does not exist |
| `GraphQueryFailed(String)` | KnowledgeGraph | Pathfinding or traversal error |
| `ChainNotFound(Uuid)` | ChainOfThought | Referenced thought chain does not exist |
| `BacktrackNotPossible(String)` | ChainOfThought | Target step index is beyond current position |
| `Memory(MemoryError)` | All subsystems | Error from Memory Platform query |
| `Runtime(RuntimeError)` | Reasoner | Error from Runtime task execution |
| `Core(CoreError)` | All subsystems | Error from Core EventBus or Logger |

**Recovery strategies**:

- `PlanCreationFailed` degrades to a simpler plan using backward chaining from a known final state. If no plan exists at all, the goal is marked Failed and an operator-facing event is published.
- `DecisionUndecidable` falls back to a satisficing strategy that selects the first candidate meeting minimum thresholds, rather than optimizing.
- `IntentAmbiguous` publishes an event with the matched template IDs for human disambiguation; the Brain pauses goal creation until the ambiguity is resolved via an explicit disambiguation event.
- `StepExecutionFailed` triggers the Reasoner's `replan` method, which attempts up to 3 replans with increasing strategy breadth before marking the step as permanently failed.

---

## Data Flow: Goal to Execution

```
Perception              IntentRecognizer         GoalManager            Reasoner
    |                         |                       |                     |
    | 1. percept event        |                       |                     |
    |------------------------>|                       |                     |
    |                         | 2. recognize intent   |                     |
    |                         | 3. extract params     |                     |
    |                         |                       |                     |
    |                         | 4. create_goal()      |                     |
    |                         |---------------------->|                     |
    |                         |                       | 5. decompose()      |
    |                         |                       |                     |
    |                         |                       | 6. create_plan()    |
    |                         |                       |-------------------->|
    |                         |                       |                     |
    |                         |                       |    Plan             |
    |                         |                       |<--------------------|
    |                         |                       |                     |

    Reasoner             DecisionMaker           Runtime TaskManager
    |                        |                        |
    | 7. for each step:      |                        |
    |    evaluate candidates |                        |
    |----------------------->|                        |
    |    Decision            |                        |
    |<-----------------------|                        |
    |                        |                        |
    | 8. create_task()       |                        |
    |----------------------------------------------->|
    |                        |                        |
    | 9. TaskCompleted       |                        |
    |<-----------------------------------------------|
    |                        |                        |
    | 10. evaluate_step()    |                        |
    | 11. next step or done  |                        |
```

---

## Future Extensions

- **Probabilistic Reasoning**: Replace single-confidence values with probability distributions. Support Monte Carlo sampling for plan outcome prediction.
- **Case-Based Reasoning**: Store successful plans as cases. Match new goals against the case library to reuse or adapt prior solutions.
- **Emotional Models**: Add valence-arousal-dominance (VAD) state tracking for the platform "mood," influencing decision utility weights and goal priority.
- **Meta-Cognition**: Add a monitoring loop that evaluates the reasoning engine's own performance (plan accuracy, decision quality) and adjusts strategies over time.
- **Explanation Generation**: Produce human-readable explanations of decisions and plan steps using the chain-of-thought trace, targeted at operator dashboards.
- **Negotiation Protocol**: Enable multiple Brain instances to negotiate resource allocation and goal priority through a structured event protocol.
- **Knowledge Graph Persistence**: Persist the knowledge graph to the Memory Platform's long-term store with snapshot and incremental backup support.
- **Constraint Satisfaction**: Integrate a CSP solver for goals with hard constraints (deadlines, resource budgets), feeding solutions into the planner.

---

## References

- [Architecture Overview](overview.md)
- [Core Platform Deep Dive](core.md)
- [Runtime Platform Deep Dive](runtime.md)
- [Memory Platform Blueprint](memory.md)
- [Perception Platform Blueprint](perception.md)
- [Execution Platform Blueprint](execution.md)
- [Design Principles](../principles.md)
- [Roadmap](../roadmap.md)
- [Glossary](../glossary.md)
