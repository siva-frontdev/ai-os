use crate::agent_registry::AgentRegistry;
use crate::errors::{CoordinatorError, CoordinatorResult};
use crate::strategy_engine::StrategyEngine;
use crate::types::{BrainSession, BrainState, CognitiveLoad, CoordinationReport, ProcessResult};
use crate::world_model::WorldModelService;
use brain_core::budget::CognitiveBudget;
use brain_core::context::ReasoningContext;
use brain_core::delegation::{DelegationRequest, DelegationResult};
use brain_core::ids::{AgentId, GoalId, ToolCapabilityId, WorkflowId};
use brain_core::model::ResourceState;
use brain_core::strategy::ExecutionStrategy;
use brain_core::tool::{ExecutablePlan, ToolRegistry, ToolRequirement};
use brain_core::traits::ReasoningService;
use brain_core::types::{Confidence, ReasoningResult};
use brain_decision::DecisionMaker;
use brain_goals::GoalManager;
use brain_goals::types::{GoalOutcome, GoalRecord};
use brain_learning::LearningEngine;
use brain_planner::Planner;
use brain_policy::PolicyEvaluator;
use brain_reasoner::ReasoningPipeline;
use brain_reflection::Reflector;
use brain_workflow::WorkflowExecutor;
use memory_core::Timestamp;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::RwLock;
use tokio::sync::mpsc;

/// The autonomous Goal-Oriented Cognitive Engine.
///
/// BrainOrchestrator has been evolved from a request-response pipeline
/// into a continuous cognitive engine that:
///
/// 1. **Observes** the environment and updates the World Model
/// 2. **Evaluates** active goals and selects strategies
/// 3. **Plans** hierarchically (Goal → Objectives → Milestones → Tasks)
/// 4. **Executes** via the Execution Platform (delegated)
/// 5. **Reflects** on outcomes to extract lessons
/// 6. **Learns** by consolidating patterns into Knowledge
/// 7. **Repeats** the loop autonomously
pub struct BrainOrchestrator {
    goal_manager: Arc<GoalManager>,
    planner: Planner,
    reasoner: ReasoningPipeline,
    policy_evaluator: Box<dyn PolicyEvaluator + Send + Sync>,
    decision_maker: DecisionMaker,
    agent_registry: AgentRegistry,
    reflector: Reflector,
    learning_engine: LearningEngine,
    workflow_executor: WorkflowExecutor,
    reasoning_service: Arc<dyn ReasoningService>,
    world_model: WorldModelService,
    strategy_engine: StrategyEngine,
    session: RwLock<BrainSession>,
    cognitive_iteration: RwLock<u64>,
    event_tx: Option<mpsc::UnboundedSender<(String, Option<GoalId>)>>,
}

impl BrainOrchestrator {
    pub fn new(
        policy_evaluator: Box<dyn PolicyEvaluator + Send + Sync>,
        tool_registry: Arc<dyn ToolRegistry>,
        goal_store: Arc<dyn brain_goals::store::GoalStore>,
        reasoning_service: Arc<dyn ReasoningService>,
    ) -> Self {
        Self {
            goal_manager: Arc::new(GoalManager::new(goal_store)),
            planner: Planner::new(tool_registry),
            reasoner: ReasoningPipeline::new(),
            policy_evaluator,
            decision_maker: DecisionMaker::new(0.5),
            agent_registry: AgentRegistry::new(),
            reflector: Reflector::new(),
            learning_engine: LearningEngine::new(),
            workflow_executor: WorkflowExecutor::new(),
            reasoning_service,
            world_model: WorldModelService::new(),
            strategy_engine: StrategyEngine::new(),
            session: RwLock::new(BrainSession {
                session_id: uuid::Uuid::new_v4().to_string(),
                state: BrainState::Idle,
                current_goal: None,
                current_plan: None,
                current_workflow: None,
                cognitive_load: CognitiveLoad::new(0.0),
                started_at: Timestamp::now(),
                updated_at: Timestamp::now(),
                step_count: 0,
                error_count: 0,
            }),
            cognitive_iteration: RwLock::new(0),
            event_tx: None,
        }
    }

    /// Set the event channel for feedback to the event-driven runtime.
    pub fn with_event_channel(
        mut self,
        tx: mpsc::UnboundedSender<(String, Option<GoalId>)>,
    ) -> Self {
        self.event_tx = Some(tx);
        self
    }

    // ── World Model access ───────────────────────────────────

    pub fn world_model(&self) -> &WorldModelService {
        &self.world_model
    }

    // ── Strategy Engine access ───────────────────────────────

    pub fn strategy_engine(&self) -> &StrategyEngine {
        &self.strategy_engine
    }

    /// Access the goal manager (shared reference).
    pub fn goal_manager(&self) -> &Arc<GoalManager> {
        &self.goal_manager
    }

    /// Emit an event to the event-driven runtime (if connected).
    fn emit_event(&self, event_type: &str, goal_id: Option<GoalId>) {
        if let Some(ref tx) = self.event_tx {
            let _ = tx.send((event_type.to_string(), goal_id));
        }
    }

    // ── Goal lifecycle ───────────────────────────────────────

    pub async fn create_goal(
        &self,
        goal_type: &str,
        description: &str,
        priority: brain_core::types::GoalPriority,
        dependencies: Vec<GoalId>,
    ) -> CoordinatorResult<GoalRecord> {
        let goal_id = GoalId::new();
        let record = self
            .goal_manager
            .create_goal(goal_id, goal_type, description, priority, dependencies)
            .await
            .map_err(|e| CoordinatorError::Goal(e.to_string()))?;

        let mut session = self
            .session
            .write()
            .map_err(|e| CoordinatorError::Internal(e.to_string()))?;
        session.state = BrainState::GoalSetting;
        session.current_goal = Some(record.clone());
        session.updated_at = Timestamp::now();
        self.emit_event("brain.goal.created", Some(goal_id));
        Ok(record)
    }

    pub async fn activate_goal(&self, goal_id: &GoalId) -> CoordinatorResult<GoalRecord> {
        let record = self
            .goal_manager
            .activate_goal(goal_id)
            .await
            .map_err(|e| CoordinatorError::Goal(e.to_string()))?;
        self.emit_event("brain.goal.activated", Some(*goal_id));
        Ok(record)
    }

    pub async fn complete_goal(
        &self,
        goal_id: &GoalId,
        outcome: GoalOutcome,
    ) -> CoordinatorResult<GoalRecord> {
        let record = self
            .goal_manager
            .complete_goal(goal_id, outcome)
            .await
            .map_err(|e| CoordinatorError::Goal(e.to_string()))?;
        self.emit_event("brain.goal.completed", Some(*goal_id));
        Ok(record)
    }

    pub async fn fail_goal(
        &self,
        goal_id: &GoalId,
        reason: impl Into<String>,
    ) -> CoordinatorResult<GoalRecord> {
        let record = self
            .goal_manager
            .fail_goal(goal_id, reason)
            .await
            .map_err(|e| CoordinatorError::Goal(e.to_string()))?;
        self.emit_event("brain.goal.failed", Some(*goal_id));
        Ok(record)
    }

    pub async fn cancel_goal(
        &self,
        goal_id: &GoalId,
        reason: impl Into<String>,
    ) -> CoordinatorResult<GoalRecord> {
        let record = self
            .goal_manager
            .cancel_goal(goal_id, reason)
            .await
            .map_err(|e| CoordinatorError::Goal(e.to_string()))?;
        self.emit_event("brain.goal.cancelled", Some(*goal_id));
        Ok(record)
    }

    pub async fn pause_goal(
        &self,
        goal_id: &GoalId,
        reason: Option<impl Into<String>>,
    ) -> CoordinatorResult<GoalRecord> {
        let record = self
            .goal_manager
            .pause_goal(goal_id, reason)
            .await
            .map_err(|e| CoordinatorError::Goal(e.to_string()))?;
        self.emit_event("brain.goal.paused", Some(*goal_id));
        Ok(record)
    }

    pub async fn retry_goal(
        &self,
        goal_id: &GoalId,
        reason: impl Into<String>,
    ) -> CoordinatorResult<GoalRecord> {
        let record = self
            .goal_manager
            .retry_goal(goal_id, reason)
            .await
            .map_err(|e| CoordinatorError::Goal(e.to_string()))?;
        self.emit_event("brain.goal.retried", Some(*goal_id));
        Ok(record)
    }

    pub async fn merge_goals(
        &self,
        target_id: GoalId,
        source_id: GoalId,
    ) -> CoordinatorResult<GoalRecord> {
        let record = self
            .goal_manager
            .merge_goals(target_id, source_id)
            .await
            .map_err(|e| CoordinatorError::Goal(e.to_string()))?;
        self.emit_event("brain.goal.merged", Some(target_id));
        Ok(record)
    }

    pub async fn reprioritize_goal(
        &self,
        goal_id: &GoalId,
        new_priority: brain_core::types::GoalPriority,
    ) -> CoordinatorResult<GoalRecord> {
        let record = self
            .goal_manager
            .reprioritize_goal(goal_id, new_priority)
            .await
            .map_err(|e| CoordinatorError::Goal(e.to_string()))?;
        self.emit_event("brain.goal.reprioritized", Some(*goal_id));
        Ok(record)
    }

    pub async fn block_goal(
        &self,
        goal_id: &GoalId,
        blocker: impl Into<String>,
    ) -> CoordinatorResult<GoalRecord> {
        let record = self
            .goal_manager
            .block_goal(goal_id, blocker)
            .await
            .map_err(|e| CoordinatorError::Goal(e.to_string()))?;
        Ok(record)
    }

    pub async fn unblock_goal(&self, goal_id: &GoalId) -> CoordinatorResult<GoalRecord> {
        let record = self
            .goal_manager
            .unblock_goal(goal_id)
            .await
            .map_err(|e| CoordinatorError::Goal(e.to_string()))?;
        Ok(record)
    }

    pub async fn add_objective(
        &self,
        goal_id: &GoalId,
        description: impl Into<String>,
        order: u32,
    ) -> CoordinatorResult<GoalRecord> {
        let (record, _) = self
            .goal_manager
            .add_objective(goal_id, description, order)
            .await
            .map_err(|e| CoordinatorError::Goal(e.to_string()))?;
        Ok(record)
    }

    pub async fn add_milestone(
        &self,
        goal_id: &GoalId,
        objective_id: &brain_core::ids::ObjectiveId,
        description: impl Into<String>,
        order: u32,
    ) -> CoordinatorResult<GoalRecord> {
        let (record, _) = self
            .goal_manager
            .add_milestone(goal_id, objective_id, description, order)
            .await
            .map_err(|e| CoordinatorError::Goal(e.to_string()))?;
        Ok(record)
    }

    pub async fn list_goals(&self) -> CoordinatorResult<Vec<GoalRecord>> {
        self.goal_manager
            .list_goals()
            .await
            .map_err(|e| CoordinatorError::Goal(e.to_string()))
    }

    pub async fn list_active_goals(&self) -> CoordinatorResult<Vec<GoalRecord>> {
        self.goal_manager
            .list_active_goals()
            .await
            .map_err(|e| CoordinatorError::Goal(e.to_string()))
    }

    // ── Planning ─────────────────────────────────────────────

    pub async fn plan_current_goal(
        &self,
        requirements: Vec<ToolRequirement>,
        budget: &CognitiveBudget,
    ) -> CoordinatorResult<ExecutablePlan> {
        let goal_id = {
            let session = self
                .session
                .read()
                .map_err(|e| CoordinatorError::Internal(e.to_string()))?;
            session
                .current_goal
                .as_ref()
                .ok_or_else(|| CoordinatorError::Goal("no goal set".into()))?
                .goal_id
        };

        let plan = self
            .planner
            .create_plan(goal_id, requirements, budget)
            .await
            .map_err(|e| CoordinatorError::Planning(e.to_string()))?;

        let mut session = self
            .session
            .write()
            .map_err(|e| CoordinatorError::Internal(e.to_string()))?;
        session.state = BrainState::Planning;
        session.current_plan = Some(plan.clone());
        session.updated_at = Timestamp::now();
        Ok(plan)
    }

    // ── Reasoning ────────────────────────────────────────────

    pub async fn reason_about_plan(
        &self,
        plan: &ExecutablePlan,
        budget: &CognitiveBudget,
    ) -> CoordinatorResult<brain_reasoner::types::ReasoningResult> {
        {
            let mut session = self
                .session
                .write()
                .map_err(|e| CoordinatorError::Internal(e.to_string()))?;
            session.state = BrainState::Reasoning;
            session.updated_at = Timestamp::now();
        }

        let session_id = self
            .session
            .read()
            .map_err(|e| CoordinatorError::Internal(e.to_string()))?
            .session_id
            .clone();

        let ctx = ReasoningContext {
            thought_id: brain_core::ids::ThoughtId::new(),
            goal_id: plan.goal_id,
            observations: vec![],
            hypotheses_in_progress: vec![],
            constraints: vec![],
            budget: budget.clone(),
            session_id,
        };

        self.reasoner
            .reason(&ctx, budget)
            .await
            .map_err(|e| CoordinatorError::Reasoning(e.to_string()))
    }

    // ── Execution ────────────────────────────────────────────

    pub fn execute_plan(
        &self,
        plan: &ExecutablePlan,
        name: &str,
        max_retries: u32,
    ) -> CoordinatorResult<WorkflowId> {
        let exec_plan =
            ExecutablePlan::new(plan.plan_id, plan.goal_id, plan.tool_requirements.clone());

        let workflow_id = self
            .workflow_executor
            .create_workflow(name, exec_plan, max_retries)
            .map_err(|e| CoordinatorError::Workflow(e.to_string()))?;
        self.workflow_executor
            .start(&workflow_id)
            .map_err(|e| CoordinatorError::Workflow(e.to_string()))?;

        let mut session = self
            .session
            .write()
            .map_err(|e| CoordinatorError::Internal(e.to_string()))?;
        session.state = BrainState::Executing;
        session.updated_at = Timestamp::now();

        Ok(workflow_id)
    }

    // ── Reflection & Learning ────────────────────────────────

    pub fn reflect_on_workflow(
        &self,
        expected_outcome: &str,
        actual_outcome: &str,
    ) -> CoordinatorResult<brain_reflection::types::Reflection> {
        let goal_id = {
            let session = self
                .session
                .read()
                .map_err(|e| CoordinatorError::Internal(e.to_string()))?;
            session
                .current_goal
                .as_ref()
                .map(|g| g.goal_id)
                .ok_or_else(|| CoordinatorError::Goal("no goal".into()))?
        };

        let reflection = self
            .reflector
            .reflect(goal_id, expected_outcome, actual_outcome)
            .map_err(|e| CoordinatorError::Reflection(e.to_string()))?;

        let mut session = self
            .session
            .write()
            .map_err(|e| CoordinatorError::Internal(e.to_string()))?;
        session.state = BrainState::Reflecting;
        session.updated_at = Timestamp::now();
        Ok(reflection)
    }

    pub fn learn_from_reflection(
        &self,
        lesson: &brain_reflection::types::Lesson,
    ) -> CoordinatorResult<brain_learning::ConsolidationReport> {
        let report = self
            .learning_engine
            .learn_from_lesson(lesson)
            .map_err(|e| CoordinatorError::Learning(e.to_string()))?;

        // Generate closed-loop feedback and apply adjustments
        if let Ok(feedback) = self.learning_engine.generate_feedback() {
            self.apply_learning_feedback(&feedback);
        }

        let mut session = self
            .session
            .write()
            .map_err(|e| CoordinatorError::Internal(e.to_string()))?;
        session.state = BrainState::Learning;
        session.updated_at = Timestamp::now();
        Ok(report)
    }

    /// Apply learning feedback to strategy engine and planner hints.
    fn apply_learning_feedback(&self, feedback: &brain_learning::LearningFeedback) {
        // Apply strategy adjustments
        for adj in &feedback.strategy_adjustments {
            let strategy = match adj.suggested_strategy.as_str() {
                "direct" => Some(brain_core::strategy::ExecutionStrategy::Direct),
                "sequential" => Some(brain_core::strategy::ExecutionStrategy::Sequential),
                "parallel" => Some(brain_core::strategy::ExecutionStrategy::Parallel),
                "research_then_execute" => {
                    Some(brain_core::strategy::ExecutionStrategy::ResearchThenExecute)
                }
                "iterative" => Some(brain_core::strategy::ExecutionStrategy::Iterative),
                "delegated" => Some(brain_core::strategy::ExecutionStrategy::Delegated),
                _ => None,
            };
            if let Some(s) = strategy {
                self.strategy_engine.set_rule(&adj.goal_type, s);
            }
        }

        // Record cognitive health in world model
        self.world_model.add_fact(
            &format!("cognitive_health: {:.2}", feedback.cognitive_health),
            "learning_engine",
            feedback.cognitive_health as f32,
        );
    }

    /// Generate and return current learning feedback.
    pub fn get_learning_feedback(&self) -> CoordinatorResult<brain_learning::LearningFeedback> {
        self.learning_engine
            .generate_feedback()
            .map_err(|e| CoordinatorError::Learning(e.to_string()))
    }

    // ── Autonomous Cognitive Loop ────────────────────────────

    /// Execute one full tick of the autonomous cognitive loop.
    ///
    /// This is the core of the Goal-Oriented Cognitive Engine.
    /// Each tick:
    ///  1. Observes the environment (updates World Model)
    ///  2. Evaluates active goals
    ///  3. Plans next actions for active goals
    ///  4. Executes plans
    ///  5. Reflects on results
    ///  6. Learns from reflection
    pub async fn cognitive_tick(&self) -> CoordinatorResult<u64> {
        let iteration = {
            let mut iter = self
                .cognitive_iteration
                .write()
                .map_err(|e| CoordinatorError::Internal(e.to_string()))?;
            *iter += 1;
            *iter
        };

        let resource_state = self.world_model.resource_state();
        if resource_state == ResourceState::Exhausted {
            return Ok(iteration);
        }

        // 1. Observe & evaluate active goals
        let active_goals = self
            .goal_manager
            .list_active_goals()
            .await
            .map_err(|e| CoordinatorError::Goal(e.to_string()))?;

        for goal in &active_goals {
            let priority_val = match goal.priority {
                brain_core::types::GoalPriority::Critical => 8,
                brain_core::types::GoalPriority::High => 6,
                brain_core::types::GoalPriority::Normal => 4,
                brain_core::types::GoalPriority::Low => 2,
            };

            // 2. Select strategy
            let (profile, approach) =
                self.strategy_engine
                    .select_strategy(&goal.goal_type, priority_val, resource_state);

            self.goal_manager
                .assign_strategy(&goal.goal_id, profile.strategy_id, approach)
                .await
                .map_err(|e| CoordinatorError::Goal(e.to_string()))?;

            // 3. Decompose hierarchically if needed
            if goal.objectives.is_empty() && goal.status == brain_core::types::GoalStatus::Active {
                self.goal_manager
                    .add_objective(&goal.goal_id, &goal.description, 0)
                    .await
                    .map_err(|e| CoordinatorError::Goal(e.to_string()))?;
            }

            // 4. Execute based on strategy
            match approach {
                ExecutionStrategy::Direct | ExecutionStrategy::Sequential => {
                    // Will be picked up by process or next tick for execution
                }
                ExecutionStrategy::Delegated => {
                    // Find available agent by goal type
                    if let Some((agent_id, _role)) =
                        self.agent_registry.find_by_capability(&goal.goal_type)
                    {
                        let delegation_id = GoalId::new();
                        let request = DelegationRequest {
                            delegation_id,
                            goal_id: goal.goal_id,
                            agent_id,
                            task_description: goal.description.clone(),
                            expected_outcome: goal.description.clone(),
                            timeout_ms: 60000,
                            priority: priority_val,
                        };
                        // Fire-and-forget delegation (result handled in future ticks)
                        let _ = self.agent_registry.delegate(&agent_id, request).await;
                    }
                }
                ExecutionStrategy::HumanAssisted => {
                    // Escalate to human interface - placeholder
                }
                _ => {}
            }
        }

        // 5. Generate learning feedback and apply adjustments
        if let Ok(feedback) = self.learning_engine.generate_feedback() {
            self.apply_learning_feedback(&feedback);
        }

        // 6. Update session state
        {
            let mut session = self
                .session
                .write()
                .map_err(|e| CoordinatorError::Internal(e.to_string()))?;
            session.step_count += 1;
            session.updated_at = Timestamp::now();
            session.cognitive_load =
                CognitiveLoad::new((active_goals.len() as f64) / 10.0_f64.max(1.0));
        }

        Ok(iteration)
    }

    /// Targeted cognitive processing for a single goal.
    ///
    /// Unlike `cognitive_tick()` which processes all active goals,
    /// this method focuses on one specific goal — ideal for
    /// event-driven wake-ups.
    pub async fn cognitive_tick_for(&self, goal_id: GoalId) -> CoordinatorResult<()> {
        let resource_state = self.world_model.resource_state();
        if resource_state == ResourceState::Exhausted {
            return Ok(());
        }

        let goal = self
            .goal_manager
            .get_goal(&goal_id)
            .await
            .map_err(|e| CoordinatorError::Goal(e.to_string()))?;

        // Only process active goals
        if goal.status != brain_core::types::GoalStatus::Active
            && goal.status != brain_core::types::GoalStatus::Pending
        {
            return Ok(());
        }

        let priority_val = match goal.priority {
            brain_core::types::GoalPriority::Critical => 8,
            brain_core::types::GoalPriority::High => 6,
            brain_core::types::GoalPriority::Normal => 4,
            brain_core::types::GoalPriority::Low => 2,
        };

        // Select strategy for this goal
        let (profile, approach) =
            self.strategy_engine
                .select_strategy(&goal.goal_type, priority_val, resource_state);

        self.goal_manager
            .assign_strategy(&goal.goal_id, profile.strategy_id, approach)
            .await
            .map_err(|e| CoordinatorError::Goal(e.to_string()))?;

        // Decompose if needed
        if goal.objectives.is_empty() && goal.status == brain_core::types::GoalStatus::Active {
            self.goal_manager
                .add_objective(&goal.goal_id, &goal.description, 0)
                .await
                .map_err(|e| CoordinatorError::Goal(e.to_string()))?;
        }

        // Delegate if the strategy requires it
        if approach == ExecutionStrategy::Delegated {
            if let Some((agent_id, _role)) = self.agent_registry.find_by_capability(&goal.goal_type)
            {
                let delegation_id = GoalId::new();
                let request = DelegationRequest {
                    delegation_id,
                    goal_id: goal.goal_id,
                    agent_id,
                    task_description: goal.description.clone(),
                    expected_outcome: goal.description.clone(),
                    timeout_ms: 60000,
                    priority: priority_val,
                };
                let _ = self.agent_registry.delegate(&agent_id, request).await;
            }
        }

        Ok(())
    }

    /// Evaluate the health of a goal's plan.
    ///
    /// Returns `Ok(())` if the plan is healthy.
    /// Returns `Err` with details if the plan needs repair.
    pub async fn evaluate_plan_health(&self, goal_id: GoalId) -> CoordinatorResult<()> {
        let goal = self
            .goal_manager
            .get_goal(&goal_id)
            .await
            .map_err(|e| CoordinatorError::Goal(e.to_string()))?;

        // A goal with no strategy assigned needs planning
        if goal.strategy_id.is_none() {
            return Err(CoordinatorError::Planning("no strategy assigned".into()));
        }

        // A failed goal needs repair
        if goal.status == brain_core::types::GoalStatus::Failed {
            return Err(CoordinatorError::Planning(format!(
                "goal {} is in failed state",
                goal_id
            )));
        }

        Ok(())
    }

    /// Repair a goal's plan by reassigning strategy and retrying.
    ///
    /// Returns `Ok(true)` if repair was performed, `Ok(false)` if
    /// no repair was needed.
    pub async fn repair_plan(&self, goal_id: GoalId) -> CoordinatorResult<bool> {
        let goal = self
            .goal_manager
            .get_goal(&goal_id)
            .await
            .map_err(|e| CoordinatorError::Goal(e.to_string()))?;

        // Only repair failed or strategy-less goals
        if goal.status != brain_core::types::GoalStatus::Failed && goal.strategy_id.is_some() {
            return Ok(false);
        }

        // Try an alternative strategy
        let resource_state = self.world_model.resource_state();
        let priority_val = match goal.priority {
            brain_core::types::GoalPriority::Critical => 8,
            brain_core::types::GoalPriority::High => 6,
            brain_core::types::GoalPriority::Normal => 4,
            brain_core::types::GoalPriority::Low => 2,
        };
        let (profile, _approach) =
            self.strategy_engine
                .select_strategy(&goal.goal_type, priority_val, resource_state);

        self.goal_manager
            .assign_strategy(&goal.goal_id, profile.strategy_id, profile.approach)
            .await
            .map_err(|e| CoordinatorError::Goal(e.to_string()))?;

        // If the goal was failed, retry it
        if goal.status == brain_core::types::GoalStatus::Failed {
            if goal.retry_count >= 3 {
                return Err(CoordinatorError::Goal(
                    "max retries exceeded for repair".into(),
                ));
            }
            self.goal_manager
                .retry_goal(&goal_id, "plan repair")
                .await
                .map_err(|e| CoordinatorError::Goal(e.to_string()))?;
        }

        self.emit_event("brain.goal.repaired", Some(goal_id));
        Ok(true)
    }

    // ── Agent delegation ──────────────────────────────────────

    pub fn agent_registry(&self) -> &AgentRegistry {
        &self.agent_registry
    }

    pub async fn delegate_to_agent(
        &self,
        agent_id: AgentId,
        goal_id: GoalId,
        task: &str,
        expected_outcome: &str,
        timeout_ms: u64,
    ) -> CoordinatorResult<DelegationResult> {
        let request = DelegationRequest {
            delegation_id: GoalId::new(),
            goal_id,
            agent_id,
            task_description: task.into(),
            expected_outcome: expected_outcome.into(),
            timeout_ms,
            priority: 5,
        };
        self.agent_registry
            .delegate(&agent_id, request)
            .await
            .map_err(|e| CoordinatorError::Internal(e.to_string()))
    }

    // ── User request processing ──────────────────────────────

    /// Build a single ToolRequirement from a capability name and matching entity.
    fn build_requirement(cap: &str, entity: Option<(&str, &str)>) -> ToolRequirement {
        let mut inputs = std::collections::HashMap::new();
        inputs.insert(
            "__cap".into(),
            brain_core::tool::ToolValue::String(cap.to_string()),
        );
        if let Some((key, value)) = entity {
            inputs.insert(
                key.to_string(),
                brain_core::tool::ToolValue::String(value.to_string()),
            );
        }
        ToolRequirement::new(ToolCapabilityId::new(), inputs, vec![])
    }

    pub fn required_capabilities_to_requirements(
        reasoning: &ReasoningResult,
    ) -> Vec<ToolRequirement> {
        if reasoning.required_capabilities.is_empty() {
            return vec![];
        }

        // Build a lookup: entity_type → first matching entity value
        let mut by_type: HashMap<&str, &str> = HashMap::new();
        for entity in &reasoning.entities {
            if !entity.entity_type.is_empty() {
                by_type
                    .entry(entity.entity_type.as_str())
                    .or_insert(entity.value.as_str());
            }
            if !entity.name.is_empty() {
                by_type
                    .entry(entity.name.as_str())
                    .or_insert(entity.value.as_str());
            }
        }

        reasoning
            .required_capabilities
            .iter()
            .map(|cap| {
                let entity = Self::find_entity_for_capability(cap, &by_type);
                Self::build_requirement(cap, entity)
            })
            .collect()
    }

    fn find_entity_for_capability<'a>(
        cap: &str,
        by_type: &HashMap<&'a str, &'a str>,
    ) -> Option<(&'a str, &'a str)> {
        // Map capability names to expected entity keys
        let needs = match cap {
            "browser.open_url" | "open_website" => Some("url"),
            "desktop.app.launch" | "launch_app" => Some("app"),
            "desktop.window.focus" | "focus_window" => Some("window"),
            "input.keyboard.type" | "type_text" => Some("text"),
            "input.mouse.click" | "mouse_click" => Some("button"),
            "search_web" | "search" => Some("query"),
            _ => None,
        };
        let key = needs?;
        if let Some(value) = by_type.get(key) {
            return Some((key, value));
        }
        // Fallback: return any entity value
        by_type.iter().next().map(|(k, v)| (*k, *v))
    }

    pub async fn process_user_input(&self, input: &str) -> CoordinatorResult<ProcessResult> {
        // 1. Structured reasoning from the Intelligence Platform
        let reasoning = self
            .reasoning_service
            .reason(input)
            .await
            .map_err(|e| CoordinatorError::Goal(e.to_string()))?;

        // 2. Create a Brain goal using the structured fields
        let goal = self
            .create_goal(
                &reasoning.intent,
                &reasoning.suggested_goal,
                reasoning.suggested_priority,
                vec![],
            )
            .await?;

        // 3. Register goal type in World Model
        let mut goal_type_hint = reasoning.intent.clone();
        if goal_type_hint.is_empty() {
            goal_type_hint = "unknown".into();
        }

        // 4. Build tool requirements from reasoning output
        let budget = CognitiveBudget::default();
        let requirements: Vec<ToolRequirement> =
            Self::required_capabilities_to_requirements(&reasoning);

        // 5. Select and assign strategy
        let resource_state = self.world_model.resource_state();
        let priority_val = match reasoning.suggested_priority {
            brain_core::types::GoalPriority::Critical => 8,
            brain_core::types::GoalPriority::High => 6,
            brain_core::types::GoalPriority::Normal => 4,
            brain_core::types::GoalPriority::Low => 2,
        };
        let (profile, _approach) =
            self.strategy_engine
                .select_strategy(&goal_type_hint, priority_val, resource_state);
        self.goal_manager
            .assign_strategy(&goal.goal_id, profile.strategy_id, profile.approach)
            .await
            .map_err(|e| CoordinatorError::Goal(e.to_string()))?;

        // 6. Plan
        let plan = self.plan_current_goal(requirements, &budget).await?;

        // 7. Reason about the plan
        let _reasoning_pipeline = self.reason_about_plan(&plan, &budget).await?;

        // 8. Execute
        let workflow_id = if plan.tool_requirements.is_empty() {
            None
        } else {
            match self.execute_plan(&plan, &goal_type_hint, 3) {
                Ok(wid) => {
                    let mut session = self
                        .session
                        .write()
                        .map_err(|e| CoordinatorError::Internal(e.to_string()))?;
                    session.current_workflow = self.workflow_executor.get_workflow(&wid).ok();
                    let wf_id = Some(wid);
                    drop(session);
                    wf_id
                }
                Err(e) => return Err(e),
            }
        };

        // 9. Run cognitive tick
        let _tick = self.cognitive_tick().await?;

        let session = self
            .session
            .read()
            .map_err(|e| CoordinatorError::Internal(e.to_string()))?;

        let result = ProcessResult {
            summary: reasoning.summary.clone(),
            reasoning_result: reasoning,
            goal_id: goal.goal_id,
            plan_id: plan.plan_id,
            workflow_id,
            state: session.state.clone(),
        };

        Ok(result)
    }

    // ── Proactive execution ─────────────────────────────────

    /// Retry all failed goals that haven't exceeded their retry limit.
    ///
    /// Returns the list of goal IDs that were retried.
    pub async fn proactive_retry_failed(&self) -> CoordinatorResult<Vec<GoalId>> {
        let failed = self
            .goal_manager
            .list_goals()
            .await
            .map_err(|e| CoordinatorError::Goal(e.to_string()))?;

        let mut retried = Vec::new();

        for goal in &failed {
            if goal.status != brain_core::types::GoalStatus::Failed {
                continue;
            }
            if goal.retry_count >= 3 {
                // Maximum retries exhausted — skip
                continue;
            }
            match self
                .goal_manager
                .retry_goal(&goal.goal_id, "auto-retry by runtime")
                .await
            {
                Ok(r) => {
                    retried.push(r.goal_id);
                }
                Err(e) => {
                    eprintln!("[proactive] failed to retry goal {}: {e}", goal.goal_id);
                }
            }
        }

        Ok(retried)
    }

    /// Discover and apply alternative strategies for goals that
    /// failed with a specific strategy.
    pub async fn proactive_alternative_strategy(
        &self,
        goal_id: &GoalId,
    ) -> CoordinatorResult<bool> {
        let goal = self
            .goal_manager
            .get_goal(goal_id)
            .await
            .map_err(|e| CoordinatorError::Goal(e.to_string()))?;

        if goal.status != brain_core::types::GoalStatus::Failed {
            return Ok(false);
        }

        // Get current strategy and try an alternative
        let resource_state = self.world_model.resource_state();
        let priority_val = match goal.priority {
            brain_core::types::GoalPriority::Critical => 8,
            brain_core::types::GoalPriority::High => 6,
            brain_core::types::GoalPriority::Normal => 4,
            brain_core::types::GoalPriority::Low => 2,
        };
        let (profile, _approach) =
            self.strategy_engine
                .select_strategy(&goal.goal_type, priority_val, resource_state);

        self.goal_manager
            .assign_strategy(&goal.goal_id, profile.strategy_id, profile.approach)
            .await
            .map_err(|e| CoordinatorError::Goal(e.to_string()))?;

        self.goal_manager
            .retry_goal(goal_id, "alternative strategy")
            .await
            .map_err(|e| CoordinatorError::Goal(e.to_string()))?;

        Ok(true)
    }

    /// Proactive repair: recover goals that are in a recoverable state
    /// and attempt to continue execution.
    pub async fn proactive_repair(&self) -> CoordinatorResult<Vec<GoalId>> {
        let mut repaired = Vec::new();

        // Retry all failed goals within retry limit
        let retried = self.proactive_retry_failed().await?;
        repaired.extend(retried);

        Ok(repaired)
    }

    // ── Session / Reporting ──────────────────────────────────

    pub fn get_session(&self) -> CoordinatorResult<BrainSession> {
        let session = self
            .session
            .read()
            .map_err(|e| CoordinatorError::Internal(e.to_string()))?;
        Ok(session.clone())
    }

    pub fn generate_report(&self) -> CoordinatorResult<CoordinationReport> {
        let session = self
            .session
            .read()
            .map_err(|e| CoordinatorError::Internal(e.to_string()))?;
        Ok(CoordinationReport {
            session_id: session.session_id.clone(),
            goal_id: session.current_goal.as_ref().map(|g| g.goal_id),
            plan_id: session.current_plan.as_ref().map(|p| p.plan_id),
            workflow_id: session.current_workflow.as_ref().map(|w| w.workflow_id),
            state: session.state.clone(),
            confidence: Confidence::new(0.8),
            summary: format!(
                "session in state {:?} with {} steps, {} goals",
                session.state,
                session.step_count,
                session.cognitive_load.raw()
            ),
            duration_ms: 0,
        })
    }
}
