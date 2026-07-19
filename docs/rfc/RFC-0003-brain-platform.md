# RFC-0003: Brain Platform

| Field | Value |
|---|---|
| **Status** | Draft |
| **Author** | Project maintainers |
| **Phase** | Phase 6 |
| **Created** | 2025-02-15 |
| **Updated** | 2025-02-15 |
| **Requires** | RFC-0001 (System Platform), RFC-0002 (Memory Platform) |
| **Supersedes** | None |

## Abstract

This RFC proposes the Brain Platform for the AI-native OS, organized as Phase 6 of the implementation roadmap. The Brain Platform provides an integrated reasoning and decision-making layer that decomposes high-level goals into executable plans, evaluates outcomes in a planner-executor-evaluator loop, selects between alternative actions using configurable decision strategies, manages a persistent goal tree with priority and dependency tracking, maintains a knowledge graph of entities and relations backed by long-term memory, and recognizes intent from natural language or structured input. It depends on the Memory Platform (Phase 5) for knowledge persistence and retrieval, the Runtime Platform (Phase 3) for task execution and scheduling, and the System Platform (Phase 4) for process and service management. All subsystems are observable by design: every reasoning step, decision, goal transition, and graph mutation emits events with trace context for audit.

## Motivation

An AI-native OS must do more than store and retrieve information; it must reason about goals, make decisions under uncertainty, and adapt its behavior based on outcomes. Without a dedicated Brain Platform, each agent or subsystem would implement ad-hoc planning logic, hardcoded decision policies, and bespoke goal tracking -- leading to architectural drift, inconsistent audit trails, and the inability to coordinate across subsystems.

The AI Lifecycle defined in `specification.md` section 7 (Act -> Observe -> Learn -> Sleep) requires a structured reasoning substrate. The Act state demands a planner that can decompose goals into steps and dispatch them as Runtime tasks. The Observe state requires an evaluator that compares outcomes against expectations. The Learn state feeds observations back into the knowledge graph and goal manager.

The complete cognitive loop (plan -> execute -> evaluate -> replan) must be a platform primitive, not an application concern. Every agent, from system management to user-facing interactions, relies on the same reasoning infrastructure.

### Specification Cross-References

- `specification.md` section 7 (AI Lifecycle): The Brain implements the Act->Observe->Learn loop for all agents.
- `specification.md` section 12 (Module Contracts): The Brain follows all module contract requirements (Service trait, EventBus, HealthMonitor).
- `specification.md` section 4 (Security Goals): KnowledgeGraph operations are subject to PermissionChecker; all decisions are logged for audit.
- `architecture/brain.md`: High-level architectural blueprint with trait definitions.
- `modules/brain.md`: Module-level decomposition across submodules.

## Design

### Overview

The Brain Platform is composed of five subsystems, each with a distinct responsibility:

```
                 ┌─────────────────────────────────────────┐
                 │              IntentRecognizer            │
                 │  Maps input -> Intent (pattern + ML)     │
                 └────────────┬────────────────────────────┘
                              │ RecognizedIntent
                              v
                 ┌─────────────────────────────────────────┐
                 │              GoalManager                 │
                 │  Goal tree CRUD, progress, persistence   │
                 └────────────┬────────────────────────────┘
                              │ Goal decomposition
                              v
    ┌─────────────────────────────────────────────────────────────┐
    │                     ReasoningEngine                         │
    │  ┌──────────┐    ┌──────────┐    ┌──────────┐              │
    │  │ Planner  │───>│ Executor │───>│Evaluator │              │
    │  │(strategy)│    │(Runtime) │    │(compare) │              │
    │  └──────────┘    └──────────┘    └──────────┘              │
    │         ^                                      │           │
    │         └────────── replan loop ───────────────┘           │
    └─────────────────────────────────────────────────────────────┘
                              │ Consults
                              v
    ┌─────────────────────────────────────────────────────────────┐
    │                     DecisionMaker                           │
    │  Utility-based | Rule-based | Cost-benefit | Consensus      │
    └────────────┬────────────────────────────────────────────────┘
                 │ Consults
                 v
    ┌─────────────────────────────────────────────────────────────┐
    │                    KnowledgeGraph                           │
    │  Entities | Relations | Facts | Path Traversal | Matching   │
    │  Backed by LongTermStorage (Memory Platform)                │
    └─────────────────────────────────────────────────────────────┘
```

All five subsystems are wired together by `BrainPlatform`, which implements the Core `Service` trait and orchestrates lifecycle, event subscriptions, and health reporting.

### Subsystem 1: ReasoningEngine

The ReasoningEngine implements the planner-executor-evaluator cognitive loop. It accepts a goal, generates a plan consisting of discrete actions, dispatches each action as a Runtime task, collects the result, evaluates it against the expected outcome, and replans if the result deviates from expectations.

#### Pluggable Reasoner Trait

```rust
use thiserror::Error;
use async_trait::async_trait;

#[derive(Error, Debug)]
pub enum BrainError {
    #[error("planning failure for goal {goal_id}: {detail}")]
    PlanningFailure { goal_id: String, detail: String },
    #[error("goal not found: {goal_id}")]
    GoalNotFound { goal_id: String },
    #[error("cycle detected in goal dependency graph: {cycle:?}")]
    CycleDetected { cycle: Vec<String> },
    #[error("step execution failed: {detail}")]
    StepExecutionFailed { step_id: String, detail: String },
    #[error("replan failed after {attempts} attempts: {detail}")]
    ReplanFailed { attempts: u32, detail: String },
    #[error("decision undecidable: {detail}")]
    DecisionUndecidable { detail: String },
    #[error("intent not recognized: {detail}")]
    IntentNotRecognized { detail: String },
    #[error("intent ambiguous: matches templates {templates:?}")]
    IntentAmbiguous { templates: Vec<String> },
    #[error("entity not found: {entity_id}")]
    EntityNotFound { entity_id: String },
    #[error("graph query failed: {detail}")]
    GraphQueryFailed { detail: String },
    #[error("permission denied: {detail}")]
    PermissionDenied { detail: String },
    #[error("memory error: {0}")]
    Memory(#[from] memory::MemoryError),
    #[error("runtime error: {0}")]
    Runtime(#[from] runtime::RuntimeError),
    #[error("core error: {0}")]
    Core(#[from] core::CoreError),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ReasoningStrategy {
    MeansEndsAnalysis,
    ForwardChaining,
    BackwardChaining,
}

#[derive(Debug, Clone)]
pub struct Plan {
    pub id: String,
    pub goal_id: String,
    pub strategy: ReasoningStrategy,
    pub steps: Vec<PlanStep>,
    pub status: PlanStatus,
    pub created_at: i64,
    pub trace_id: String,
}

#[derive(Debug, Clone)]
pub struct PlanStep {
    pub id: String,
    pub action: String,
    pub parameters: Vec<(String, String)>,
    pub dependencies: Vec<String>,
    pub expected_outcome: String,
    pub status: StepStatus,
    pub task_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PlanStatus { Pending, InProgress, Completed, Failed, Replanned }

#[derive(Debug, Clone, PartialEq)]
pub enum StepStatus { Pending, Ready, Running, Succeeded, Failed, Skipped }

#[derive(Debug, Clone)]
pub struct StepOutcome {
    pub step_id: String,
    pub success: bool,
    pub observation: String,
    pub metrics: HashMap<String, f64>,
    pub duration_ms: u64,
}

#[derive(Debug, Clone)]
pub struct Evaluation {
    pub step_id: String,
    pub verdict: Verdict,
    pub deviation: Option<String>,
    pub confidence: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Verdict { OnTrack, OffTrack, Blocked }

#[async_trait]
pub trait Reasoner: Debug + Send + Sync {
    async fn create_plan(
        &self,
        goal_id: &str,
        strategy: ReasoningStrategy,
        context: &GoalContext,
    ) -> Result<Plan, BrainError>;
    async fn execute_step(&self, step_id: &str) -> Result<StepOutcome, BrainError>;
    async fn evaluate_step(
        &self,
        step_id: &str,
        outcome: &StepOutcome,
    ) -> Result<Evaluation, BrainError>;
    async fn replan(
        &self,
        plan_id: &str,
        failed_step_id: &str,
        evaluation: &Evaluation,
    ) -> Result<Plan, BrainError>;
    async fn get_plan(&self, plan_id: &str) -> Result<Plan, BrainError>;
    fn active_plans(&self) -> Vec<String>;
}
```

#### Loop Mechanics

The cognitive loop runs as an async Tokio task per reasoning session:

```
1. planner.plan(goal_id, strategy, context) -> Plan
2. For each step in plan.steps (respecting dependencies):
   a. step.status = Running
   b. emit PlanStepStarted event
   c. task_id = task_manager.spawn(step.action, step.parameters)
   d. await TaskCompleted(task_id) event
   e. outcome = StepOutcome { success: true, observation, ... }
   f. step.status = Succeeded (or Failed)
   g. evaluation = evaluator.evaluate(step_id, &outcome)
   h. emit PlanStepCompleted / PlanStepFailed event
   i. if evaluation.verdict != OnTrack:
        plan = replan(plan_id, step_id, &evaluation)
        emit PlanReplanned event
        goto step 2
3. plan.status = Completed
   emit PlanCompleted event
```

The loop is driven by Runtime task completion events (`runtime.task_completed`, `runtime.task_failed`). The executor dispatches actions through the TaskManager by constructing a `TaskHandle` with the action payload and awaiting its result via an event-driven callback.

#### Pluggable Strategy Framework

```rust
#[async_trait]
pub trait PlannerStrategy: Debug + Send + Sync {
    fn name(&self) -> &str;
    async fn decompose(
        &self,
        goal: &Goal,
        knowledge: &dyn KnowledgeGraph,
    ) -> Result<Vec<PlanStep>, BrainError>;
}

pub struct MeansEndsPlanner;
pub struct ForwardChainingPlanner;
pub struct BackwardChainingPlanner;

impl PlannerStrategy for MeansEndsPlanner {
    fn name(&self) -> &str { "means_ends" }
    async fn decompose(
        &self,
        goal: &Goal,
        knowledge: &dyn KnowledgeGraph,
    ) -> Result<Vec<PlanStep>, BrainError> {
        // Identify difference between current state and goal state.
        // Select operators that reduce the difference.
        // Recursively decompose until primitive actions remain.
        todo!()
    }
}
```

The `Reasoner` holds a `HashMap<String, Arc<dyn PlannerStrategy>>` and selects a strategy based on goal type or context.

### Subsystem 2: DecisionMaker

The DecisionMaker selects between alternative courses of action using configurable strategies. It is consulted by the Reasoner when multiple valid actions exist for a given step and by the GoalManager when priority conflicts arise.

#### Core Types

```rust
#[derive(Debug, Clone)]
pub struct DecisionRequest {
    pub request_id: String,
    pub context: HashMap<String, f64>,
    pub candidates: Vec<Candidate>,
    pub strategy: DecisionStrategy,
    pub trace_id: String,
}

#[derive(Debug, Clone)]
pub struct Candidate {
    pub id: String,
    pub label: String,
    pub estimated_cost: f64,
    pub estimated_benefit: f64,
    pub estimated_risk: f64,
    pub estimated_duration_ms: u64,
    pub resource_demand: ResourceDemand,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct ResourceDemand {
    pub cpu_cores: f64,
    pub memory_mb: u64,
    pub network_bps: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DecisionStrategy {
    UtilityMaximize,
    RuleBased,
    CostBenefit,
    Consensus,
}

#[derive(Debug, Clone)]
pub struct Decision {
    pub request_id: String,
    pub selected: Candidate,
    pub strategy: DecisionStrategy,
    pub rationale: String,
    pub scores: Vec<(String, f64)>,
    pub confidence: f64,
    pub timestamp: i64,
}

#[async_trait]
pub trait DecisionMaker: Debug + Send + Sync {
    async fn decide(&self, request: DecisionRequest) -> Result<Decision, BrainError>;
    async fn evaluate_utility(&self, candidates: &[Candidate], weights: &UtilityWeights) -> Vec<f64>;
    async fn evaluate_rules(&self, candidates: &[Candidate], context: &DecisionRequest) -> Vec<Candidate>;
    async fn cost_benefit(&self, a: &[Candidate], b: &[Candidate]) -> Result<CBAReport, BrainError>;
}

#[derive(Debug, Clone)]
pub struct UtilityWeights {
    pub cost: f64,
    pub benefit: f64,
    pub risk: f64,
    pub speed: f64,
    pub resource_efficiency: f64,
}

impl Default for UtilityWeights {
    fn default() -> Self {
        Self { cost: 0.3, benefit: 0.4, risk: 0.2, speed: 0.05, resource_efficiency: 0.05 }
    }
}

#[derive(Debug, Clone)]
pub struct CBAReport {
    pub option_a_cost: f64,
    pub option_a_benefit: f64,
    pub option_b_cost: f64,
    pub option_b_benefit: f64,
    pub net_difference: f64,
    pub confidence: f64,
    pub breakdown: HashMap<String, (f64, f64)>,
}
```

#### Strategy Behaviors

| Strategy | Behavior | When to Use |
|---|---|---|
| **UtilityMaximize** | Score each candidate using weighted linear utility function. Select highest score. | Default. Quantitative attributes available (cost, duration, risk). |
| **RuleBased** | Apply ordered list of if-then rules. First rule whose conditions match determines selection. | Policy-driven environments. Compliance requirements. |
| **CostBenefit** | Compare weighted pros and cons of each alternative. Net benefit determines selection. | Strategic decisions with long-term impact. |
| **Consensus** | Run all three strategies above. Apply majority vote or average ranking. Tie-breaking by utility score. | High-stakes decisions requiring cross-validation. |

All decisions are logged with the full rationale string, candidate scores, and strategy name for audit compliance (see `specification.md` section 4.4).

#### DecisionMaker Implementation

```rust
#[derive(Debug)]
pub struct DefaultDecisionMaker {
    utility_weights: Arc<RwLock<UtilityWeights>>,
    rules: Arc<Vec<PolicyRule>>,
    knowledge_graph: Arc<dyn KnowledgeGraph>,
    memory_retriever: Arc<dyn MemoryRetriever>,
    event_bus: Arc<dyn EventBus>,
}

#[derive(Debug, Clone)]
pub struct PolicyRule {
    pub name: String,
    pub condition: RuleCondition,
    pub action: RuleAction,
    pub priority: u32,
}

pub enum RuleCondition {
    Always,
    ContextMatches { key: String, pattern: String },
    Threshold { metric: String, operator: Comparison, value: f64 },
    GraphQuery { query: String },
}

pub enum RuleAction {
    SelectCandidate(String),
    FilterOut { reason: String },
    AdjustWeight { dimension: String, multiplier: f64 },
}

impl DefaultDecisionMaker {
    async fn strategy_utility(&self, candidates: &[Candidate], weights: &UtilityWeights) -> Vec<f64> {
        candidates.iter().map(|c| {
            weights.cost * (1.0 - c.estimated_cost)
                + weights.benefit * c.estimated_benefit
                + weights.risk * (1.0 - c.estimated_risk)
                + weights.speed * (1.0 - c.estimated_duration_ms as f64 / 1_000_000.0)
                + weights.resource_efficiency * resource_score(&c.resource_demand)
        }).collect()
    }
}
```

### Subsystem 3: GoalManager

The GoalManager maintains a tree of goals with parent-child decomposition, priority ordering, deadline tracking, dependency management, and progress computation. Goals are persisted to the Memory Platform's LongTermStorage for recovery after restart.

#### Core Types

```rust
#[derive(Debug, Clone)]
pub struct Goal {
    pub id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub description: String,
    pub state: GoalState,
    pub priority: u8,
    pub owner_session: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub deadline: Option<i64>,
    pub progress: f64,
    pub subgoal_ids: Vec<String>,
    pub dependencies: Vec<String>,
    pub result: Option<GoalResult>,
    pub trace_id: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GoalState {
    Active,
    InProgress,
    Suspended,
    Completed,
    Failed,
    Abandoned,
}

#[derive(Debug, Clone)]
pub struct GoalResult {
    pub summary: String,
    pub success: bool,
    pub metrics: HashMap<String, f64>,
    pub artifacts: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct GoalSpec {
    pub name: String,
    pub description: String,
    pub priority: u8,
    pub deadline: Option<i64>,
    pub dependencies: Vec<String>,
}

#[async_trait]
pub trait GoalManager: Debug + Send + Sync {
    async fn create_goal(
        &self,
        name: &str,
        description: &str,
        owner_session: &str,
        priority: u8,
        deadline: Option<i64>,
    ) -> Result<Goal, BrainError>;
    async fn decompose_goal(
        &self,
        parent_id: &str,
        subgoals: Vec<GoalSpec>,
    ) -> Result<Vec<String>, BrainError>;
    async fn update_state(&self, goal_id: &str, state: GoalState) -> Result<Goal, BrainError>;
    async fn update_progress(&self, goal_id: &str, progress: f64) -> Result<(), BrainError>;
    async fn get_goal(&self, goal_id: &str) -> Result<Goal, BrainError>;
    async fn get_goal_tree(&self, root_id: &str) -> Result<Vec<Goal>, BrainError>;
    async fn find_dependency_cycle(&self, goal_ids: &[String]) -> Result<Option<Vec<String>>, BrainError>;
    async fn set_result(&self, goal_id: &str, result: GoalResult) -> Result<(), BrainError>;
    fn list_active_goals(&self) -> Vec<String>;
    async fn restore_from_memory(&self) -> Result<usize, BrainError>;
}

#[derive(Debug)]
pub struct DefaultGoalManager {
    goals: RwLock<HashMap<String, Goal>>,
    children: RwLock<HashMap<String, Vec<String>>>,
    long_term: Arc<dyn LongTermStorage>,
    event_bus: Arc<dyn EventBus>,
    scheduler: Arc<dyn Scheduler>,
}
```

#### Persistence

Goals are serialized as JSON entries and stored in the Memory Platform's LongTermStorage under a dedicated column family or key prefix (`brain:goal:{goal_id}`). On startup, `restore_from_memory()` loads all goals with state != Completed/Failed/Abandoned and rebuilds the in-memory goal tree. Goals in InProgress state are reset to Active on recovery (the associated plans are gone), and a `GoalStateChanged` event is emitted with previous and new states.

```rust
impl DefaultGoalManager {
    const GOAL_KEY_PREFIX: &'static str = "brain:goal:";

    async fn persist_goal(&self, goal: &Goal) -> Result<(), BrainError> {
        let key = format!("{}{}", Self::GOAL_KEY_PREFIX, goal.id);
        let value = serde_json::to_vec(goal)
            .map_err(|e| BrainError::Core(core::CoreError::Serialization(e.to_string())))?;
        let entry = memory::MemoryEntry {
            key,
            value,
            content_type: "application/json".into(),
            session_id: goal.owner_session.clone(),
            timestamp: goal.updated_at,
            ttl: None,
            importance: goal.priority as f32 / 255.0,
            tags: vec!["brain:goal".into(), goal.state.as_str().into()],
            ..Default::default()
        };
        self.long_term.store(entry).await?;
        Ok(())
    }

    async fn load_active_goals(&self) -> Result<Vec<Goal>, BrainError> {
        let session_ids = self.list_known_sessions().await?;
        let mut goals = Vec::new();
        for session_id in session_ids {
            let keys = self.long_term
                .list_by_prefix(&format!("{}{}:", Self::GOAL_KEY_PREFIX, session_id))
                .await?;
            for key in keys {
                let entry = self.long_term.retrieve(&key, &session_id).await?;
                if let Ok(goal) = serde_json::from_slice::<Goal>(&entry.value) {
                    if matches!(goal.state, GoalState::Active | GoalState::InProgress | GoalState::Suspended) {
                        goals.push(goal);
                    }
                }
            }
        }
        Ok(goals)
    }
}
```

#### Dependency Cycle Detection

Cycle detection uses DFS with a visited set and recursion stack. The `find_dependency_cycle` method returns the first cycle found or None.

```rust
impl DefaultGoalManager {
    async fn detect_cycle(&self, start_id: &str) -> Option<Vec<String>> {
        let goals = self.goals.read().await;
        let mut visited = HashSet::new();
        let mut stack = Vec::new();
        let mut path = Vec::new();

        fn dfs<'a>(
            id: &'a str,
            goals: &'a HashMap<String, Goal>,
            visited: &mut HashSet<&'a str>,
            stack: &mut Vec<&'a str>,
            path: &mut Vec<String>,
        ) -> Option<Vec<String>> {
            if stack.contains(&id) {
                let cycle_start = stack.iter().position(|&x| x == id).unwrap();
                return Some(stack[cycle_start..].iter().map(|s| s.to_string()).collect());
            }
            if visited.contains(id) {
                return None;
            }
            visited.insert(id);
            stack.push(id);
            if let Some(goal) = goals.get(id) {
                for dep in &goal.dependencies {
                    if let Some(cycle) = dfs(dep, goals, visited, stack, path) {
                        return Some(cycle);
                    }
                }
            }
            stack.pop();
            None
        }

        dfs(start_id, &goals, &mut visited, &mut stack, &mut path)
    }
}
```

### Subsystem 4: KnowledgeGraph

The KnowledgeGraph stores typed entities, binary relations, and asserted facts. It uses `petgraph::StableGraph` for in-memory operations and serializes to the Memory Platform's LongTermStorage for persistence. Graph operations are subject to the PermissionChecker.

#### Core Types

```rust
use petgraph::stable_graph::{StableGraph, NodeIndex, EdgeIndex};
use petgraph::Directed;

#[derive(Debug, Clone)]
pub struct Entity {
    pub id: String,
    pub entity_type: String,
    pub name: String,
    pub attributes: HashMap<String, String>,
    pub created_at: i64,
    pub session_id: String,
}

#[derive(Debug, Clone)]
pub struct Relation {
    pub id: String,
    pub relation_type: String,
    pub source_id: String,
    pub target_id: String,
    pub weight: f64,
    pub confidence: f64,
    pub attributes: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct Fact {
    pub subject_id: String,
    pub predicate: String,
    pub object_id: String,
    pub confidence: f64,
    pub source: String,
    pub timestamp: i64,
}

#[derive(Debug, Clone)]
pub struct Subgraph {
    pub entities: Vec<Entity>,
    pub relations: Vec<Relation>,
    pub depth: u8,
}

#[derive(Debug, Clone)]
pub struct PathSegment {
    pub from_entity: Entity,
    pub relation: Relation,
    pub to_entity: Entity,
    pub weight: f64,
}

#[async_trait]
pub trait KnowledgeGraph: Debug + Send + Sync {
    async fn add_entity(&self, entity: Entity) -> Result<String, BrainError>;
    async fn add_relation(&self, relation: Relation) -> Result<String, BrainError>;
    async fn assert_fact(&self, fact: Fact) -> Result<(), BrainError>;
    async fn retract_fact(&self, subject_id: &str, predicate: &str, object_id: &str) -> Result<(), BrainError>;
    async fn get_entity(&self, entity_id: &str) -> Result<Entity, BrainError>;
    async fn find_entities(&self, entity_type: &str, name: &str) -> Result<Vec<Entity>, BrainError>;
    async fn find_path(&self, from_id: &str, to_id: &str, max_depth: u8) -> Result<Vec<PathSegment>, BrainError>;
    async fn match_subgraph(&self, pattern: &SubgraphPattern) -> Result<Vec<Subgraph>, BrainError>;
    async fn neighbors(&self, entity_id: &str, max_depth: u8) -> Result<Subgraph, BrainError>;
    async fn snapshot(&self) -> Result<Vec<u8>, BrainError>;
    async fn restore(&self, data: &[u8]) -> Result<(), BrainError>;
}

#[derive(Debug, Clone)]
pub struct SubgraphPattern {
    pub entities: Vec<EntityPattern>,
    pub relations: Vec<RelationPattern>,
}

#[derive(Debug, Clone)]
pub struct EntityPattern {
    pub entity_type: Option<String>,
    pub name_contains: Option<String>,
    pub attribute_match: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
pub struct RelationPattern {
    pub relation_type: Option<String>,
    pub source_pattern: Option<Box<EntityPattern>>,
    pub target_pattern: Option<Box<EntityPattern>>,
    pub weight_min: Option<f64>,
}
```

#### Implementation

```rust
#[derive(Debug)]
pub struct DefaultKnowledgeGraph {
    graph: RwLock<StableGraph<EntityData, RelationData, Directed>>,
    entity_index: RwLock<HashMap<String, NodeIndex>>,
    type_index: RwLock<HashMap<String, Vec<NodeIndex>>>,
    long_term: Arc<dyn LongTermStorage>,
    permission_checker: Arc<dyn PermissionChecker>,
    event_bus: Arc<dyn EventBus>,
    config: KnowledgeGraphConfig,
}

#[derive(Debug, Clone)]
struct EntityData {
    id: String,
    entity_type: String,
    name: String,
    attributes: HashMap<String, String>,
    created_at: i64,
    session_id: String,
}

#[derive(Debug, Clone)]
struct RelationData {
    id: String,
    relation_type: String,
    weight: f64,
    confidence: f64,
    attributes: HashMap<String, String>,
}
```

Graph construction and queries:

```rust
impl DefaultKnowledgeGraph {
    async fn check_permission(&self, session_id: &str, operation: &str) -> Result<(), BrainError> {
        self.permission_checker
            .check_access(session_id, &format!("knowledge:{}", operation))
            .map_err(|e| BrainError::PermissionDenied { detail: e.to_string() })
    }

    pub async fn find_shortest_path(
        &self,
        from_id: &str,
        to_id: &str,
        max_depth: u8,
    ) -> Result<Vec<PathSegment>, BrainError> {
        let graph = self.graph.read().await;
        let entity_idx = self.entity_index.read().await;
        let from = entity_idx.get(from_id).ok_or_else(|| BrainError::EntityNotFound {
            entity_id: from_id.to_string(),
        })?;
        let to = entity_idx.get(to_id).ok_or_else(|| BrainError::EntityNotFound {
            entity_id: to_id.to_string(),
        })?;
        let path = petgraph::algo::astar(
            &*graph,
            *from,
            |n| n == *to,
            |e| e.weight().weight,
            |_| 0.0,
        ).ok_or_else(|| BrainError::GraphQueryFailed {
            detail: format!("no path from {} to {} within max_depth {}", from_id, to_id, max_depth),
        })?;
        let segments = path.1.windows(2).filter_map(|w| {
            let edge = graph.find_edge(w[0], w[1])?;
            let rel = graph.edge_weight(edge)?;
            let from_e = graph.node_weight(w[0])?;
            let to_e = graph.node_weight(w[1])?;
            Some(PathSegment {
                from_entity: entity_from_data(from_e),
                relation: relation_from_data(rel),
                to_entity: entity_from_data(to_e),
                weight: rel.weight,
            })
        }).collect();
        Ok(segments)
    }

    pub async fn find_subgraphs(
        &self,
        pattern: &SubgraphPattern,
    ) -> Result<Vec<Subgraph>, BrainError> {
        let graph = self.graph.read().await;
        let entity_idx = self.entity_index.read().await;
        let type_idx = self.type_index.read().await;
        let mut results = Vec::new();
        // For each entity type in the pattern, find candidate nodes.
        // For each candidate, perform BFS matching the relation pattern.
        // Collect matching subgraphs with depth <= max pattern depth.
        // This is a simplified subgraph isomorphism (VF2 is deferred).
        todo!()
    }
}
```

#### Persistence Strategy

| Aspect | Decision |
|---|---|
| **In-memory format** | `petgraph::StableGraph<NodeId=usize>` for O(1) node/edge lookup and path algorithms |
| **Serialization format** | Bincode for the graph structure; JSON for entity/relation metadata |
| **Storage backend** | LongTermStorage from Memory Platform, key prefix `brain:kg:` |
| **Snapshot interval** | Every 100 mutations or every 60 seconds, whichever comes first |
| **Recovery** | On `start()`, load latest snapshot and replay write-ahead log of mutations since the snapshot |

### Subsystem 5: IntentRecognizer

The IntentRecognizer maps natural language or structured input to typed intents. It uses rule-based pattern matching as the primary path, with a deferred hook for ML-based classification (Phase 9).

#### Core Types

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum IntentType {
    Query,
    Command,
    Create,
    Update,
    Delete,
    Configure,
    Investigate,
}

#[derive(Debug, Clone)]
pub struct RecognizedIntent {
    pub intent_type: IntentType,
    pub confidence: f64,
    pub parameters: HashMap<String, String>,
    pub matched_template: String,
    pub raw_input: String,
}

#[derive(Debug, Clone)]
pub struct IntentTemplate {
    pub id: String,
    pub name: String,
    pub intent_type: IntentType,
    pub patterns: Vec<String>,
    pub parameter_extractors: Vec<ParameterExtractor>,
    pub min_confidence: f64,
}

#[derive(Debug, Clone)]
pub struct ParameterExtractor {
    pub name: String,
    pub pattern: String,
    pub required: bool,
    pub default_value: Option<String>,
}

#[async_trait]
pub trait IntentRecognizer: Debug + Send + Sync {
    async fn register_template(&self, template: IntentTemplate) -> Result<(), BrainError>;
    async fn recognize(&self, input: &str) -> Result<Option<RecognizedIntent>, BrainError>;
    async fn disambiguate(&self, input: &str, template_ids: &[String]) -> Result<RecognizedIntent, BrainError>;
    fn list_templates(&self) -> Vec<IntentTemplate>;
    fn attach_ml_classifier(&self, classifier: Arc<dyn MlClassifier>);
}

#[async_trait]
pub trait MlClassifier: Debug + Send + Sync {
    async fn classify(&self, input: &str) -> Result<Vec<(IntentType, f64)>, BrainError>;
    fn is_available(&self) -> bool;
}
```

#### Pattern Matching Pipeline

```
Input -> Normalize -> Match patterns (regex) -> Score -> Threshold check -> Extract params
```

Each template contains a list of regex patterns. A template matches if any pattern matches the normalized input. The match score is the maximum regex match quality across all patterns (currently binary: match=1.0, no-match=0.0). When ML classification is available (Phase 9), the pipeline becomes:

```
Input -> Rule-based match -> confidence >= min_confidence? ->
  Yes -> Extract parameters -> return RecognizedIntent
  No  -> ML classifier -> merge scores -> consensus -> return
```

If input matches multiple templates above their confidence thresholds, the recognizer returns `IntentAmbiguous` with the list of matching template IDs. The caller (or an upstream disambiguation handler) calls `disambiguate()` with additional context.

### Integration Points

#### ReasoningEngine -> TaskManager

The Reasoner's executor dispatches actions as Runtime tasks:

```rust
impl DefaultReasoner {
    async fn dispatch_step(&self, step: &PlanStep) -> Result<StepOutcome, BrainError> {
        let task = runtime::TaskHandle {
            action: step.action.clone(),
            parameters: step.parameters.clone().into_iter().collect(),
            priority: runtime::Priority::Normal,
            session_id: self.session_id.clone(),
            trace_id: self.trace_id.clone(),
            ..Default::default()
        };
        let task_id = self.task_manager.spawn(task).await
            .map_err(|e| BrainError::Runtime(e))?;
        // Await completion via event subscription or oneshot channel
        let outcome = self.await_task_completion(&task_id).await?;
        Ok(outcome)
    }
}
```

#### DecisionMaker -> KnowledgeGraph + MemoryRetriever

The DecisionMaker consults the KnowledgeGraph for context-dependent rule evaluation and the MemoryRetriever for historical precedent:

```rust
impl DefaultDecisionMaker {
    async fn enrich_context(&self, request: &DecisionRequest) -> Result<DecisionRequest, BrainError> {
        let mut enriched = request.clone();
        // Query KG for entities related to the candidates
        for candidate in &enriched.candidates {
            let neighbors = self.knowledge_graph
                .neighbors(&candidate.id, 1).await.ok();
            if let Some(subgraph) = neighbors {
                enriched.context.insert(
                    format!("kg_neighbors_{}", candidate.id),
                    subgraph.entities.len() as f64,
                );
            }
        }
        // Query Memory for similar past decisions
        let query = memory::QueryRequest {
            query_text: format!("decision: {}", request.request_id),
            session_id: "*".into(),
            top_k: 5,
            ..Default::default()
        };
        if let Ok(result) = self.memory_retriever.query(query).await {
            enriched.context.insert("memory_results".into(), result.entries.len() as f64);
        }
        Ok(enriched)
    }
}
```

#### GoalManager -> LongTermStorage

Goal persistence uses the LongTermStorage `MemoryStore` trait from the Memory Platform (see RFC-0002). Goals are serialized as JSON and stored with the key prefix `brain:goal:`.

#### IntentRecognizer -> Perception Platform

The IntentRecognizer is designed to receive input from the Perception Platform (Phase 7). A `perception.percept.classified` event carries the raw text or structured payload. Until Perception is implemented, the IntentRecognizer exposes a direct `recognize(&str)` method for testing and manual invocation.

#### Event Emissions

All subsystems emit events for observability, audit, and cross-module coordination.

**Published Events:**

| Event | Type String | Subsystem | Payload |
|---|---|---|---|
| `PlanCreated` | `brain.plan.created` | Reasoner | `{ plan_id, goal_id, strategy, step_count, trace_id }` |
| `PlanStepStarted` | `brain.plan.step_started` | Reasoner | `{ plan_id, step_id, action, parameters }` |
| `PlanStepCompleted` | `brain.plan.step_completed` | Reasoner | `{ plan_id, step_id, success, duration_ms, observation }` |
| `PlanStepFailed` | `brain.plan.step_failed` | Reasoner | `{ plan_id, step_id, error, attempt }` |
| `PlanReplanned` | `brain.plan.replanned` | Reasoner | `{ plan_id, failed_step_id, new_strategy, step_count }` |
| `PlanCompleted` | `brain.plan.completed` | Reasoner | `{ plan_id, goal_id, total_steps, total_duration_ms }` |
| `DecisionMade` | `brain.decision.made` | DecisionMaker | `{ request_id, selected_id, strategy, rationale, scores }` |
| `GoalCreated` | `brain.goal.created` | GoalManager | `{ goal_id, name, priority, owner_session }` |
| `GoalDecomposed` | `brain.goal.decomposed` | GoalManager | `{ parent_id, subgoal_ids }` |
| `GoalStateChanged` | `brain.goal.state_changed` | GoalManager | `{ goal_id, previous_state, new_state, reason }` |
| `GoalCompleted` | `brain.goal.completed` | GoalManager | `{ goal_id, result_summary, duration_ms }` |
| `GoalFailed` | `brain.goal.failed` | GoalManager | `{ goal_id, reason, failed_subgoals }` |
| `GoalAbandoned` | `brain.goal.abandoned` | GoalManager | `{ goal_id, reason }` |
| `IntentRecognized` | `brain.intent.recognized` | IntentRecognizer | `{ intent_type, confidence, parameters, raw_input }` |
| `IntentAmbiguous` | `brain.intent.ambiguous` | IntentRecognizer | `{ matched_templates, raw_input }` |
| `EntityAdded` | `brain.knowledge.entity_added` | KnowledgeGraph | `{ entity_id, entity_type, name }` |
| `RelationAdded` | `brain.knowledge.relation_added` | KnowledgeGraph | `{ relation_id, relation_type, source_id, target_id }` |
| `FactAsserted` | `brain.knowledge.fact_asserted` | KnowledgeGraph | `{ subject_id, predicate, object_id, confidence }` |
| `FactRetracted` | `brain.knowledge.fact_retracted` | KnowledgeGraph | `{ subject_id, predicate, object_id }` |

**Consumed Events:**

| Event | Source | Consumer | Action |
|---|---|---|---|
| `runtime.task_completed` | Runtime | Reasoner | Advance plan step, trigger evaluation |
| `runtime.task_failed` | Runtime | Reasoner | Trigger replan or mark goal failed |
| `memory.item.retrieved` | Memory | KnowledgeGraph | Update entity access recency |
| `memory.consolidation.completed` | Memory | GoalManager | Re-check goals against new knowledge |
| `system.resource.warning` | System | DecisionMaker | Adjust utility weights under pressure |

### Thread Model

| Subsystem | State | Concurrency Primitive | Notes |
|---|---|---|---|
| Reasoner | `HashMap<String, Plan>` | `RwLock` | One reasoning session per Tokio task. Multiple sessions run concurrently. |
| DecisionMaker | `UtilityWeights`, `Vec<PolicyRule>` | `RwLock` (weights), `Arc<Vec>` (rules) | Stateless evaluation per request. Rules are immutable after load. |
| GoalManager | `HashMap<String, Goal>` + children index | `RwLock` | Read-heavy (goal tree queries). Writes on create/update/decompose. |
| KnowledgeGraph | `StableGraph` + 2 index maps | `RwLock` | Path traversal under read lock. Mutations under write lock. |
| IntentRecognizer | `HashMap<String, IntentTemplate>` | `RwLock` | Read-heavy. Templates registered at start, rarely modified at runtime. |

No dedicated background threads. The goal monitoring periodic task runs on the shared Tokio runtime via `tokio::spawn`. All blocking I/O (Memory Platform writes) uses `spawn_blocking`.

### Lifecycle

`BrainPlatform` implements the Core `Service` trait:

```
init():
  1. Validate that Memory Platform and Runtime Platform are available (via Container).
  2. Create subsystem instances (KnowledgeGraph, GoalManager, IntentRecognizer, DecisionMaker, Reasoner).
  3. Load intent templates from configuration.
  4. Register health checks for each subsystem.
  5. Subscribe to consumed events.

start():
  1. KnowledgeGraph.restore() -- load snapshot from LongTermStorage.
  2. GoalManager.restore_from_memory() -- reload active goals.
  3. Register periodic goal monitoring task (every 5 seconds, checks deadlines, propagates progress).
  4. Emit brain.started event.

stop():
  1. Cancel goal monitoring task.
  2. GoalManager persists all active goals to LongTermStorage.
  3. KnowledgeGraph snapshot to LongTermStorage.
  4. Unsubscribe from all events.
  5. Emit brain.stopped event.
```

#### Start Order Within the Brain

```
1. KnowledgeGraph      -- required by all other subsystems
2. GoalManager         -- required by Reasoner
3. IntentRecognizer    -- standalone, loads templates early
4. DecisionMaker       -- depends on KnowledgeGraph for context enrichment
5. Reasoner            -- depends on Goals, Knowledge, and DecisionMaker
6. Event subscriptions -- registered last to avoid processing events before subsystems are ready
```

### Error Handling

All errors use the unified `BrainError` enum (defined in Subsystem 1 above).

| Variant | Subsystem | Recovery |
|---|---|---|
| `PlanningFailure` | Reasoner | Fall back to simpler strategy (forward->backward->means-ends). If all fail, mark goal Failed. |
| `StepExecutionFailed` | Reasoner | Retry up to 3 times. After 3 failures, replan around the failed step. If replan fails, mark goal Failed. |
| `ReplanFailed` | Reasoner | Emit `PlanFailed` event. GoalManager marks goal as Failed with error detail. |
| `DecisionUndecidable` | DecisionMaker | Fall back to satisficing (select first candidate meeting minimum thresholds). |
| `GoalNotFound` | GoalManager | Return error to caller. Caller should validate goal_id before operations. |
| `CycleDetected` | GoalManager | Reject the dependency. Log at WARN level. Return cycle path to caller for resolution. |
| `EntityNotFound` | KnowledgeGraph | Return error to caller for validation. |
| `GraphQueryFailed` | KnowledgeGraph | Retry once. On failure, return error with detail for diagnostic. |
| `IntentNotRecognized` | IntentRecognizer | Return None. Caller may request clarification or fall back to default behavior. |
| `IntentAmbiguous` | IntentRecognizer | Emit `IntentAmbiguous` event with matched template IDs. Await disambiguation. |
| `PermissionDenied` | All | Log at WARN. Return error to caller. |
| `Memory` / `Runtime` / `Core` | All | Propagate original error with context. Respect upstream retry policies. |

### Security Considerations

- **Authorization**: Every KnowledgeGraph mutation (`add_entity`, `add_relation`, `assert_fact`) calls `PermissionChecker.check_access(session_id, "knowledge:{operation}")`. Read operations (`get_entity`, `find_path`) also check read permission.
- **Audit**: Every decision is logged with the full rationale, candidate scores, and strategy name. Goal state transitions are persisted and emitted as events. All events carry trace context for end-to-end audit trails.
- **Session Isolation**: Goals are scoped to their owner session. GoalManager queries filter by `owner_session`. KnowledgeGraph entities carry a `session_id` field for access control.
- **No new attack surface**: The Brain runs in-process and communicates exclusively through the EventBus. It introduces no network listeners or external API endpoints.

### Performance Considerations

| Operation | Target Latency | Notes |
|---|---|---|
| `IntentRecognizer.recognize()` | < 100 µs | Regex matching on short strings. ML path (Phase 9) will be < 10 ms. |
| `GoalManager.create_goal()` | < 50 µs | HashMap insertion + event emission. |
| `GoalManager.get_goal_tree()` | < 200 µs | BFS traversal of shallow trees (depth < 20). |
| `KnowledgeGraph.get_entity()` | < 5 µs | HashMap lookup by ID. |
| `KnowledgeGraph.find_path()` | < 1 ms | A* on StableGraph with up to 10K nodes. |
| `DecisionMaker.decide()` | < 500 µs | Utility computation for < 100 candidates. |
| `Reasoner.create_plan()` | < 10 ms | Strategy-specific decomposition. Forward/backward chaining with < 50 steps. |

### Testing Strategy

**Unit tests** (per subsystem):

- Reasoner: Plan creation with each strategy, step execution with mock TaskManager, evaluation of on/off-track outcomes, replan triggering.
- DecisionMaker: Utility scoring correctness, rule-based filtering, cost-benefit report accuracy, consensus tie-breaking.
- GoalManager: CRUD operations, tree traversal, progress propagation, dependency cycle detection, persistence round-trip.
- KnowledgeGraph: Entity/relation CRUD, path finding (including no-path case), subgraph matching, snapshot/restore round-trip.
- IntentRecognizer: Exact match, partial match below threshold, multiple matches (ambiguity), parameter extraction.

**Integration tests**:

- Full cognitive loop: Recognize intent -> Create goal -> Plan -> Execute steps (mock Runtime) -> Evaluate -> Complete. Verify event sequence matches expected order.
- Replan loop: Step fails -> Evaluate -> Replan -> New plan executed. Verify planner is called with failed step context.
- Goal persistence: Create goals -> Stop Brain -> Restart Brain -> Verify goals recovered with correct state.
- KnowledgeGraph persistence: Add entities -> Snapshot -> Restore -> Query entities.

**Property-based tests**:

- Goal dependency graph: For any acyclic graph, topological sort completes. For any graph with a cycle, `find_dependency_cycle` returns a valid cycle.
- KnowledgeGraph: `add_entity` -> `get_entity` returns the same entity. `add_relation` creates a traversable edge.
- DecisionMaker: Utility function is monotonic with respect to each weight dimension.

**Performance benchmarks**:

- KnowledgeGraph path finding: A* vs BFS vs Dijkstra on graphs of varying sizes (100, 1K, 10K nodes).
- DecisionMaker: Utility evaluation throughput (candidates/sec).
- GoalManager: Tree traversal latency at various depths (5, 10, 20 levels).

### Dependencies

| Dependency | Layer | Purpose |
|---|---|---|
| `ai_os_core` | Core | EventBus, Service trait, Logger, HealthMonitor, LifecycleManager |
| `ai_os_runtime` | Runtime | TaskManager, Scheduler, Supervisor, PermissionChecker, ContextManager |
| `ai_os_memory` | Memory | LongTermStorage for goal/KG persistence, MemoryRetriever for decision context |
| `petgraph` | External | Directed graph data structure for KnowledgeGraph and goal dependencies |
| `serde` + `serde_json` | External | Serialization of goals, plans, entities, relations |
| `regex` | External | Pattern matching in IntentRecognizer |
| `uuid` | External | Identifiers for plans, goals, entities, relations |
| `chrono` | External | Timestamps for goals, plan steps, decisions |
| `thiserror` | External | Error type derivation for BrainError |
| `async-trait` | External | Async trait methods for Reasoner, DecisionMaker, GoalManager, etc. |
| `bincode` | External | Compact binary serialization for KnowledgeGraph snapshots |

### Configuration

```toml
[brain.reasoning]
max_plan_steps = 50
retry_limit = 3
replan_strategy_order = ["means_ends", "forward_chaining", "backward_chaining"]

[brain.decision]
default_strategy = "utility_maximize"
utility_weights = { cost = 0.3, benefit = 0.4, risk = 0.2, speed = 0.05, resource = 0.05 }
enable_consensus_for_high_stakes = true

[brain.goals]
max_active_goals = 50
persistence_interval_secs = 30
goal_monitoring_interval_secs = 5
recover_in_progress_as_active = true

[brain.knowledge]
snapshot_interval_mutations = 100
snapshot_interval_secs = 60
enable_type_index = true

[brain.intent]
min_confidence = 0.7
ml_fallback_enabled = false
```

## Drawbacks

1. **Cognitive loop complexity**: The planner-executor-evaluator loop introduces a stateful async execution model that can be hard to debug. Plan state is distributed across the Reasoner, Runtime tasks, and EventBus subscribers. Without careful instrumentation (trace context on every event), diagnosing loop failures will be difficult.
2. **petgraph dependency**: `petgraph` is a well-maintained library, but it adds a non-trivial dependency with generic algorithms (A*, Dijkstra, DFS) that must be tested for correctness with our custom data model. If petgraph's algorithm behavior diverges from expectations, we may need to implement pathfinding ourselves.
3. **Ephemeral plan state**: Plans live only in the Reasoner's in-memory HashMap. If the process crashes mid-plan, the plan is lost and the goal is reset to Active on restart. Full plan persistence would add write amplification and latency to the hot loop.
4. **Regex-based intent recognition**: Simple regex patterns will miss semantically identical inputs with different phrasings. The ML fallback (Phase 9) mitigates this, but until then, recognition accuracy is limited.
5. **Goal tree scalability**: The in-memory goal tree (HashMap + children index) works for thousands of goals. Above 100K goals, tree traversal and cycle detection will become latency-bound. A database-backed goal store is a future consideration.

## Alternatives Considered

| Alternative | Reason for Rejection |
|---|---|
| **Monolithic Reasoner** (single struct with all planning logic) | Violates single-responsibility principle. Strategy switching requires recompilation. Testing is harder without mockable traits. |
| **External planner** (PDDL solver or STRIPS planner) | Adds a subprocess dependency with non-trivial IPC overhead. PDDL is overly expressive for our use case. Hard to integrate with Rust error handling. |
| **No DecisionMaker** (always pick the first valid action) | Cannot support policy-driven environments. No audit trail for why an action was chosen. Impossible to optimize for cost, risk, or speed. |
| **Flat goal list** (no tree hierarchy) | Cannot express goal decomposition. No progress propagation from subgoals. Hard to track dependencies across related goals. |
| **SQLite-backed KnowledgeGraph** | Adds SQL dependency and schema migration burden. Graph operations (path finding, subgraph matching) are awkward and slow in SQL. `petgraph` is simpler and faster for in-memory operations. |
| **ML-only IntentRecognizer** | Rule-based matching is deterministic, auditable, and requires no model serving infrastructure. ML is useful as a fallback, not a replacement. |
| **External knowledge graph** (Neo4j, ArangoDB) | Unacceptable latency for sub-millisecond graph lookups. Network round-trips add 100us-1ms per query. The in-memory `petgraph` approach achieves target latencies. |

## Open Questions

1. **Plan persistence trade-off**: Should plans be persisted to LongTermStorage so they survive crashes? Persisting after every step would add ~1ms latency to the cognitive loop. An alternative is to persist only on suspension (when the agent enters Sleep state), accepting potential loss of in-flight plans on crash.
2. **Consensus strategy voting**: How should ties be broken in the Consensus strategy? Three options: (a) fall back to utility scores, (b) prefer the rule-based result (policy wins), (c) random selection with logged rationale. Option (a) seems most principled.
3. **KnowledgeGraph snapshot format**: `bincode` is optimal for Rust-to-Rust but creates a versioning problem if the Entity/Relation structs change. Should we version the snapshot format? A version header in the serialized data is cheap insurance.
4. **Goal priority inheritance**: When a parent goal is decomposed, should subgoals inherit the parent's priority or get their own? Inheritance simplifies the model but reduces flexibility. Current design: subgoals get their own priority (from `GoalSpec`), defaulting to the parent's priority if unspecified.
5. **Memory Platform readiness**: Phase 5 (Memory Platform) must be complete before Phase 6 can begin. The Brain depends on `LongTermStorage` for goal persistence, `MemoryRetriever` for decision context, and `MemoryEntry` for serialization. Any delays in Phase 5 will block Phase 6 integration testing.

## Implementation Plan

### Phase 6a — KnowledgeGraph (Week 1-2)

1. Define `Entity`, `Relation`, `Fact` types and `BrainError` in `brain/src/types.rs`.
2. Implement `DefaultKnowledgeGraph` with `petgraph::StableGraph`, `RwLock`, and index maps.
3. Implement `add_entity`, `add_relation`, `get_entity`, `find_entities`, `neighbors`.
4. Implement `find_path` using A* search.
5. Implement `match_subgraph` using constrained BFS.
6. Implement `snapshot` (bincode serialization) and `restore` (deserialization + graph rebuild).
7. Integrate `PermissionChecker` for all mutation operations.
8. Unit tests for all CRUD, path finding, subgraph matching, snapshot/restore.
9. Benchmarks for path finding on 100/1K/10K node graphs.
10. Update `architecture/brain.md` and `modules/brain.md` with final KnowledgeGraph details.

### Phase 6b — GoalManager (Week 3-4)

1. Define `Goal`, `GoalSpec`, `GoalState`, `GoalResult` types.
2. Implement `DefaultGoalManager` with `HashMap` + children `RwLock`.
3. Implement CRUD operations, `decompose_goal`, `update_progress`.
4. Implement `find_dependency_cycle` with DFS.
5. Implement `persist_goal` and `restore_from_memory` using LongTermStorage.
6. Integrate event emissions for all state transitions.
7. Unit tests for CRUD, tree traversal, cycle detection, persistence round-trip.
8. Property-based tests: acyclic graph property, cycle detection completeness.

### Phase 6c — IntentRecognizer (Week 4-5)

1. Define `IntentType`, `IntentTemplate`, `ParameterExtractor`, `RecognizedIntent`.
2. Implement `DefaultIntentRecognizer` with regex-based pattern matching.
3. Implement `register_template`, `recognize`, `disambiguate`.
4. Define `MlClassifier` trait with default no-op implementation.
5. Integrate with event bus for `perception.percept.classified` (prepared for Phase 7).
6. Unit tests for exact match, partial match, ambiguity, parameter extraction.
7. Golden test dataset: 50 input strings with expected intent/parameters.

### Phase 6d — DecisionMaker (Week 5-6)

1. Define `DecisionRequest`, `Candidate`, `Decision`, `UtilityWeights`, `PolicyRule`.
2. Implement `DefaultDecisionMaker` with four strategies.
3. Implement `evaluate_utility` with weighted linear scoring.
4. Implement rule evaluation engine (ordered list of condition-action rules).
5. Implement cost-benefit analysis report generation.
6. Implement consensus strategy (run all three, vote).
7. Integrate with KnowledgeGraph for context enrichment.
8. Unit tests for each strategy, edge cases (empty candidates, all filtered by rules).
9. Audit: verify every decision event carries rationale, scores, and strategy.

### Phase 6e — ReasoningEngine (Week 6-8)

1. Define `Plan`, `PlanStep`, `StepOutcome`, `Evaluation`, `Reasoner` trait.
2. Implement `PlannerStrategy` trait with three concrete strategies:
   - `MeansEndsPlanner`: compute state difference, select operator, decompose.
   - `ForwardChainingPlanner`: start from current state, apply rules until goal reached.
   - `BackwardChainingPlanner`: start from goal state, work backward to current state.
3. Implement `DefaultReasoner` with the planner-executor-evaluator loop.
4. Implement the executor: dispatch steps as Runtime tasks, await completion via events.
5. Implement the evaluator: compare outcome against expected outcome, produce Verdict.
6. Implement replan logic: retry up to 3 times, then change strategy, then fail.
7. Wire all subsystems into `BrainPlatform` Service implementation.
8. Integration test: full cognitive loop with mock Runtime and Memory.
9. Recovery test: crash mid-plan, restart, verify goal is recovered as Active.
10. Update `architecture/brain.md` and `modules/brain.md` with final designs.

### Milestones

| Milestone | Deliverables | Target |
|---|---|---|
| M1 | KnowledgeGraph with path finding, snapshots, and permission checks | End of week 2 |
| M2 | GoalManager with persistence, cycle detection, and event emissions | End of week 4 |
| M3 | IntentRecognizer with pattern matching and golden test dataset | End of week 5 |
| M4 | DecisionMaker with four strategies, audit logging, and KG integration | End of week 6 |
| M5 | ReasoningEngine with full cognitive loop and integration tests | End of week 8 |
| M6 | BrainPlatform Service lifecycle, docs updated, RFC finalized | End of week 9 |

## Unresolved Topics

- **Meta-cognition**: A monitoring loop that evaluates the reasoning engine's own performance and adjusts strategy selection, utility weights, and plan depth based on historical success rates. Deferred to a follow-up RFC.
- **Multi-agent coordination**: Multiple Brain instances coordinating via shared goals and knowledge graph. Requires distributed consensus and is deferred until multi-node support.
- **Explanation generation**: Producing human-readable natural-language explanations of decisions and plan steps from the chain-of-thought trace. Targeted at operator dashboards. Deferred to Phase 9 (Intelligence Integration).
- **Knowledge Graph versioning**: Support for branching and merging entity/relation snapshots. Useful for rollback of faulty knowledge assertions. Out of scope for initial implementation.
- **Probabilistic reasoning**: Replacing single-confidence values with probability distributions. Support for Monte Carlo sampling for plan outcome prediction. Deferred.
- **Episodic memory replay**: Using consolidated memories from the Memory Platform to inform planning, analogous to hippocampal replay in biological brains. Requires Memory Platform maturity.
- **Counterfactual reasoning**: Simulating alternative action sequences offline to evaluate "what if" scenarios. Deferred.

## References

1. `specification.md` -- Section 7: AI Lifecycle (Act, Observe, Learn, Sleep states).
2. `specification.md` -- Section 4: Security Goals (authorization, audit requirements).
3. `specification.md` -- Section 12: Module Contracts (Service trait, EventBus, HealthMonitor).
4. `architecture/brain.md` -- High-level architectural blueprint with all trait definitions.
5. `modules/brain.md` -- Module-level decomposition across submodules.
6. `architecture/runtime.md` -- Runtime Platform traits (TaskManager, Scheduler, PermissionChecker).
7. `architecture/memory.md` -- Memory Platform traits (LongTermStorage, MemoryRetriever).
8. RFC-0001 -- System Platform (filesystem abstraction, process management).
9. RFC-0002 -- Memory Platform (long-term storage, retrieval, indexing).
10. `petgraph` crate: <https://crates.io/crates/petgraph>
11. `regex` crate: <https://crates.io/crates/regex>
12. `bincode` crate: <https://crates.io/crates/bincode>
