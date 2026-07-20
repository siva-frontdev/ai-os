use crate::errors::{CoordinatorError, CoordinatorResult};
use crate::types::{BrainSession, BrainState, CognitiveLoad, CoordinationReport};
use brain_core::budget::CognitiveBudget;
use brain_core::context::ReasoningContext;
use brain_core::ids::{GoalId, WorkflowId};
use brain_core::tool::{ExecutablePlan, ToolRegistry, ToolRequirement};
use brain_core::types::Confidence;
use brain_decision::DecisionMaker;
use brain_goals::GoalManager;
use brain_goals::types::GoalRecord;
use brain_learning::LearningEngine;
use brain_planner::Planner;
use brain_policy::PolicyEvaluator;
use brain_reasoner::ReasoningPipeline;
use brain_reflection::Reflector;
use brain_workflow::WorkflowExecutor;
use memory_core::Timestamp;
use std::sync::Arc;
use std::sync::RwLock;

pub struct BrainOrchestrator {
    goal_manager: GoalManager,
    planner: Planner,
    reasoner: ReasoningPipeline,
    #[allow(dead_code)]
    policy_evaluator: Box<dyn PolicyEvaluator + Send + Sync>,
    #[allow(dead_code)]
    decision_maker: DecisionMaker,
    reflector: Reflector,
    learning_engine: LearningEngine,
    workflow_executor: WorkflowExecutor,
    session: RwLock<BrainSession>,
}

impl BrainOrchestrator {
    pub fn new(
        policy_evaluator: Box<dyn PolicyEvaluator + Send + Sync>,
        tool_registry: Arc<dyn ToolRegistry>,
        goal_store: Arc<dyn brain_goals::store::GoalStore>,
    ) -> Self {
        Self {
            goal_manager: GoalManager::new(goal_store),
            planner: Planner::new(tool_registry),
            reasoner: ReasoningPipeline::new(),
            policy_evaluator,
            decision_maker: DecisionMaker::new(0.5),
            reflector: Reflector::new(),
            learning_engine: LearningEngine::new(),
            workflow_executor: WorkflowExecutor::new(),
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
        }
    }

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
        Ok(record)
    }

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

        let mut session = self
            .session
            .write()
            .map_err(|e| CoordinatorError::Internal(e.to_string()))?;
        session.state = BrainState::Learning;
        session.updated_at = Timestamp::now();
        Ok(report)
    }

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
                "session in state {:?} with {} steps",
                session.state, session.step_count
            ),
            duration_ms: 0,
        })
    }
}
