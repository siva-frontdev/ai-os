use brain_core::BrainResult;
use brain_core::budget::CognitiveBudget;
use brain_core::context::DecisionContext;
use brain_core::ids::{GoalId, ToolCapabilityId, ToolId};
use brain_core::tool::ExecutablePlan;
use brain_core::tool::{ToolCandidate, ToolCapability, ToolRegistry, ToolRequirement};
use brain_core::traits::ReasoningService;
use brain_core::types::GoalPriority;
use brain_goals::InMemoryGoalStore;
use brain_goals::types::GoalRecord;
use brain_policy::PolicyEvaluator;
use brain_policy::types::PolicyEvaluation;
use std::sync::Arc;

use crate::orchestrator::BrainOrchestrator;
use crate::types::BrainState;

#[derive(Debug)]
pub(crate) struct MockReasoningService;

#[async_trait::async_trait]
impl ReasoningService for MockReasoningService {
    async fn reason(&self, input: &str) -> BrainResult<brain_core::types::ReasoningResult> {
        Ok(brain_core::types::ReasoningResult {
            summary: format!("analysis of: {input}"),
            intent: "test".into(),
            confidence: brain_core::types::Confidence::new(0.9),
            observations: vec![],
            entities: vec![],
            inferred_constraints: vec![],
            assumptions: vec![],
            hypotheses: vec![],
            selected_hypothesis: None,
            suggested_goal: format!("test objective: {input}"),
            suggested_priority: brain_core::types::GoalPriority::Normal,
            required_capabilities: vec![],
            explanation: "mock explanation".into(),
            metadata: std::collections::HashMap::new(),
        })
    }
}

pub(crate) struct MockToolRegistry;

#[async_trait::async_trait]
impl ToolRegistry for MockToolRegistry {
    async fn find_candidates(
        &self,
        _capability: ToolCapabilityId,
    ) -> BrainResult<Vec<ToolCandidate>> {
        Ok(Vec::new())
    }
    async fn register(&self, _candidate: ToolCandidate) -> BrainResult<()> {
        Ok(())
    }
    async fn deregister(&self, _tool_id: ToolId) -> BrainResult<()> {
        Ok(())
    }
    async fn list_capabilities(&self) -> BrainResult<Vec<ToolCapability>> {
        Ok(Vec::new())
    }
}

pub(crate) struct MockPolicyEvaluator;

#[async_trait::async_trait]
impl PolicyEvaluator for MockPolicyEvaluator {
    async fn applicable(
        &self,
        _policies: &[brain_policy::types::RuleSet],
        _ctx: &DecisionContext,
    ) -> brain_policy::PolicyResult<Vec<brain_policy::types::PolicyRule>> {
        Ok(Vec::new())
    }
    async fn evaluate_policy(
        &self,
        _policy: &brain_policy::types::RuleSet,
        _ctx: &DecisionContext,
        _plan: &ExecutablePlan,
    ) -> brain_policy::PolicyResult<PolicyEvaluation> {
        Ok(PolicyEvaluation {
            passed: true,
            violated_rules: vec![],
            warnings: vec![],
            allowance: 1.0,
        })
    }
    async fn aggregate(
        &self,
        _evaluations: &[PolicyEvaluation],
    ) -> brain_policy::PolicyResult<PolicyEvaluation> {
        Ok(PolicyEvaluation {
            passed: true,
            violated_rules: vec![],
            warnings: vec![],
            allowance: 1.0,
        })
    }
}

fn make_goal_record() -> GoalRecord {
    GoalRecord::new(
        GoalId::from(uuid::Uuid::from_bytes({
            let mut buf = [0u8; 16];
            buf[0] = 1;
            buf[15] = 1;
            buf
        })),
        "test",
        "test objective",
        GoalPriority::Normal,
    )
}

fn make_plan() -> ExecutablePlan {
    ExecutablePlan::new(
        brain_core::ids::PlanId::new(),
        make_goal_record().goal_id,
        vec![],
    )
}

fn make_orchestrator() -> BrainOrchestrator {
    let policy = Box::new(MockPolicyEvaluator);
    let tool_registry = Arc::new(MockToolRegistry);
    let goal_store = Arc::new(InMemoryGoalStore::new());
    let reasoning: Arc<dyn ReasoningService> = Arc::new(MockReasoningService);
    BrainOrchestrator::new(policy, tool_registry, goal_store, reasoning)
}

fn make_requirement() -> ToolRequirement {
    ToolRequirement::new(
        ToolCapabilityId::new(),
        std::collections::HashMap::new(),
        vec!["output".into()],
    )
}

#[tokio::test]
async fn test_orchestrator_set_goal() {
    let orchestrator = make_orchestrator();
    let record = orchestrator
        .create_goal("test", "test objective", GoalPriority::Normal, vec![])
        .await
        .unwrap();
    assert!(!record.description.is_empty());
    let session = orchestrator.get_session().unwrap();
    assert_eq!(session.state, BrainState::GoalSetting);
    assert!(session.current_goal.is_some());
}

#[tokio::test]
async fn test_orchestrator_plan() {
    let orchestrator = make_orchestrator();
    orchestrator
        .create_goal("test", "test objective", GoalPriority::Normal, vec![])
        .await
        .unwrap();
    let budget = CognitiveBudget::default();
    orchestrator
        .plan_current_goal(vec![make_requirement()], &budget)
        .await
        .unwrap();
    let session = orchestrator.get_session().unwrap();
    assert_eq!(session.state, BrainState::Planning);
    assert!(session.current_plan.is_some());
}

#[tokio::test]
async fn test_orchestrator_reason() {
    let orchestrator = make_orchestrator();
    orchestrator
        .create_goal("test", "test objective", GoalPriority::Normal, vec![])
        .await
        .unwrap();
    let budget = CognitiveBudget::default();
    let plan = orchestrator
        .plan_current_goal(vec![make_requirement()], &budget)
        .await
        .unwrap();
    let result = orchestrator
        .reason_about_plan(&plan, &budget)
        .await
        .unwrap();
    assert!(!result.hypotheses.is_empty() || !result.inferences.is_empty());
}

#[tokio::test]
async fn test_orchestrator_execute() {
    let orchestrator = make_orchestrator();
    let plan = make_plan();
    let _wf_id = orchestrator.execute_plan(&plan, "test", 1).unwrap();
    let session = orchestrator.get_session().unwrap();
    assert_eq!(session.state, BrainState::Executing);
}

#[tokio::test]
async fn test_orchestrator_reflect_and_learn() {
    let orchestrator = make_orchestrator();
    orchestrator
        .create_goal("test", "test objective", GoalPriority::Normal, vec![])
        .await
        .unwrap();
    let reflection = orchestrator
        .reflect_on_workflow("expected", "actual")
        .unwrap();
    assert!(!reflection.lessons.is_empty());

    if let Some(lesson) = reflection.lessons.first() {
        let report = orchestrator.learn_from_reflection(lesson).unwrap();
        assert!(report.pattern_count > 0 || !report.patterns_found.is_empty());
    }
}

#[tokio::test]
async fn test_orchestrator_generates_report() {
    let orchestrator = make_orchestrator();
    let report = orchestrator.generate_report().unwrap();
    assert!(!report.summary.is_empty());
}
