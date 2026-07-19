# Brain Platform Interface — `brain` *(planned, Phase 6)*

## Purpose

The Brain layer is the central intelligence of the OS. It owns the planner-executor-evaluator loop that drives goal-directed behavior. It receives input from Perception, queries Memory for context, issues commands through Execution, and maintains a knowledge graph of entities, relations, and goals. The Brain is the highest designed layer — layers above it (Reflection, Agency) are future work.

## Public APIs

### ReasoningEngine — inference and chain-of-thought

```rust
#[async_trait]
pub trait ReasoningEngine: Debug + Send + Sync {
    async fn infer(&self, ctx: &ReasoningContext, query: &str) -> Result<Inference, ReasoningError>;
    async fn chain(&self, ctx: &ReasoningContext, steps: Vec<ReasoningStep>) -> Result<ChainResult, ReasoningError>;
    async fn evaluate(&self, ctx: &ReasoningContext, statement: &str) -> Result<TruthValue, ReasoningError>;
    async fn explain(&self, inference_id: &InferenceId) -> Result<Explanation, ReasoningError>;
    async fn clear_cache(&self) -> Result<(), ReasoningError>;
}
```

### DecisionMaker — strategy selection and action ranking

```rust
#[async_trait]
pub trait DecisionMaker: Debug + Send + Sync {
    async fn decide(&self, ctx: &DecisionContext, options: Vec<Action>) -> Result<Decision, DecisionError>;
    async fn rank(&self, ctx: &DecisionContext, options: Vec<Action>) -> Result<Vec<ScoredAction>, DecisionError>;
    async fn strategy(&self) -> DecisionStrategy;
    async fn set_strategy(&self, strategy: DecisionStrategy) -> Result<(), DecisionError>;
    fn events(&self) -> Box<dyn Stream<Item = DecisionEvent> + Unpin + Send>;
}
```

### GoalManager — goal lifecycle and decomposition

```rust
#[async_trait]
pub trait GoalManager: Debug + Send + Sync {
    async fn create(&self, spec: GoalSpec) -> Result<GoalId, GoalError>;
    async fn cancel(&self, id: &GoalId) -> Result<(), GoalError>;
    async fn status(&self, id: &GoalId) -> Result<GoalStatus, GoalError>;
    async fn decompose(&self, id: &GoalId) -> Result<Vec<SubGoal>, GoalError>;
    async fn prioritize(&self, ids: &[GoalId], strategy: PrioritizationStrategy) -> Result<(), GoalError>;
    async fn snapshot(&self) -> Result<GoalSnapshot, GoalError>;
    fn stream(&self) -> Box<dyn Stream<Item = GoalEvent> + Unpin + Send>;
}
```

### KnowledgeGraph — entity-relation store

```rust
#[async_trait]
pub trait KnowledgeGraph: Debug + Send + Sync {
    async fn insert(&self, entity: Entity) -> Result<EntityId, GraphError>;
    async fn relate(&self, sub: &EntityId, pred: &RelationType, obj: &EntityId) -> Result<RelationId, GraphError>;
    async fn query(&self, q: GraphQuery) -> Result<GraphResult, GraphError>;
    async fn traverse(&self, start: &EntityId, depth: usize) -> Result<SubGraph, GraphError>;
    async fn merge(&self, other: SubGraph, strategy: MergeStrategy) -> Result<(), GraphError>;
    async fn stats(&self) -> Result<GraphStats, GraphError>;
}
```

### IntentRecognizer — natural language to structured intent

```rust
#[async_trait]
pub trait IntentRecognizer: Debug + Send + Sync {
    async fn recognize(&self, input: &str, ctx: &IntentContext) -> Result<Intent, IntentError>;
    async fn recognize_multi(&self, input: &str, ctx: &IntentContext) -> Result<Vec<ScoredIntent>, IntentError>;
    async fn entities(&self, input: &str) -> Result<Vec<ExtractedEntity>, IntentError>;
    async fn reload_model(&self) -> Result<(), IntentError>;
}
```

## Dependencies

- [Core](core.md) — `EventBus`, `Config`, `Logger`, `HealthMonitor`
- [Memory](memory.md) — `MemoryStore`, `MemoryRetriever` (context retrieval)
- [Perception](perception.md) *(planned)* — `EventClassifier` output (processed input)
- [Execution](execution.md) *(planned)* — `CommandEngine` (action dispatch)
- `petgraph` (knowledge graph internals)
- `tract` / `ort` (ONNX inference for intent recognition)

## Lifecycle

1. **Init** — `KnowledgeGraph` is loaded from persistent storage. `GoalManager` loads the goal registry. `IntentRecognizer` loads the NLU model. `ReasoningEngine` initializes the inference cache.
2. **Start** — The planner-executor-evaluator loop begins. `GoalManager` evaluates goal progress on a tick interval (default: 500 ms). `DecisionMaker` is idle until decisions are requested.
3. **Stop** — In-flight reasoning chains are cancelled. Goal snapshots are persisted. The planner loop exits.

## Events Published

| Event | Payload | Description |
|-------|---------|-------------|
| `brain.goal_created` | `{ id, spec, priority }` | New goal registered |
| `brain.goal_completed` | `{ id, outcome }` | Goal reached or abandoned |
| `brain.goal_blocked` | `{ id, blockers: Vec<String> }` | Goal cannot make progress |
| `brain.decision_made` | `{ context, chosen, alternatives }` | Decision with alternatives |
| `brain.inference_complete` | `{ id, confidence, latency_ms }` | Reasoning chain finished |
| `brain.knowledge_merged` | `{ entities_added, relations_added }` | External knowledge integrated |
| `brain.intent_conflict` | `{ input, candidates }` | Multiple intents with similar scores |

## Events Consumed

| Source | Event | Handling |
|--------|-------|----------|
| [Perception](perception.md) *(planned)* | `perception.sensor_reading` | `IntentRecognizer::recognize` on text input |
| [Perception](perception.md) *(planned)* | `perception.event_classified` | Feed classified event into `ReasoningEngine` |
| [Memory](memory.md) | `memory.retrieval_miss` | Trigger `GoalManager` to refine query or gather more input |
| [Core](core.md) | `core.config_changed` | Reload decision strategy, NLU model path, planner tick rate |

## Error Model

| Error | Variant | Recovery |
|-------|---------|----------|
| `ReasoningError::ModelNotLoaded` | Inference model unavailable | Request load, block until ready |
| `DecisionError::NoViableOption` | All actions scored below threshold | Return no-op, inform `GoalManager` |
| `GoalError::CircularDependency` | Goal graph contains cycle | Reject creation, return error |
| `GraphError::EntityNotFound` | Traversal target missing | Return empty subgraph |
| `IntentError::LowConfidence` | Best intent below threshold | Return `Intent::Unknown` |
| `IntentError::ModelStale` | Model version outdated | Trigger reload, use stale model |

## Performance Expectations

| Operation | Target Latency (p99) | Throughput |
|-----------|---------------------|------------|
| Intent recognition (short input) | < 10 ms | 500+/s |
| Intent recognition (long input) | < 50 ms | 200+/s |
| Knowledge graph query (local) | < 100 µs | 10K+/s |
| Knowledge graph traverse (depth 3) | < 1 ms | 5K+/s |
| Inference (simple, cached) | < 5 ms | 200+/s |
| Inference (chain, 5 steps) | < 200 ms | 50+/s |
| Decision (10 options) | < 2 ms | 500+/s |

## Thread Model

- `ReasoningEngine` dispatches model inference to `spawn_blocking` (ONNX sessions are not `Send`). Tokio tasks await results.
- `KnowledgeGraph` uses `Arc<RwLock<petgraph::Graph>>`. Queries are read-locked, mutations write-locked.
- `GoalManager` runs on a tokio interval tick. Each tick evaluates progress for active goals.
- `IntentRecognizer` keeps the model in a thread-local or `Arc<Mutex<>>` and uses `spawn_blocking` for inference.
- All public types are `Send + Sync`. The model sessions (which may not be `Sync`) are hidden behind internal locks.

## Portability

- The Brain layer is entirely portable — it depends on no OS-specific APIs.
- `ReasoningEngine` abstracts the inference backend (ONNX, llama.cpp, mock) behind a private trait.
- `IntentRecognizer` uses a portable NLU model format (ONNX). Training is done offline.
- `KnowledgeGraph` is pure Rust (`petgraph`) with optional persistence via the [Memory](memory.md) layer.
- No file paths, environment variables, or system calls are made directly.
- The planner-executor-evaluator loop is fully deterministic and can be replayed for debugging.
