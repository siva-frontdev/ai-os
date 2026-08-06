//! Deterministic stubs for Brain trait dependencies.

use std::sync::Arc;

use brain_core::delegation::{
    AgentCapability, AgentProvider, AgentRole, DelegationRequest, DelegationResult,
    DelegationStatus,
};
use brain_core::ids::{GoalId, ToolId};
use brain_core::tool::{
    ExecutablePlan, ToolCandidate, ToolCapability, ToolRegistry, ToolRequirement,
};
use brain_core::traits::ReasoningService;
use brain_core::types::ReasoningResult;
use brain_policy::traits::PolicyEvaluator;
use brain_policy::types::{PolicyEvaluation, PolicyRule, RuleSet};

/// A policy evaluator that always approves.
#[derive(Debug, Default)]
pub struct AllowAllPolicyEvaluator;

#[async_trait::async_trait]
impl PolicyEvaluator for AllowAllPolicyEvaluator {
    async fn applicable(
        &self,
        _policies: &[RuleSet],
        _ctx: &brain_core::context::DecisionContext,
    ) -> brain_policy::PolicyResult<Vec<PolicyRule>> {
        Ok(Vec::new())
    }

    async fn evaluate_policy(
        &self,
        _policy: &RuleSet,
        _ctx: &brain_core::context::DecisionContext,
        _plan: &ExecutablePlan,
    ) -> brain_policy::PolicyResult<PolicyEvaluation> {
        Ok(PolicyEvaluation {
            passed: true,
            violated_rules: Vec::new(),
            warnings: Vec::new(),
            allowance: 1.0,
        })
    }

    async fn aggregate(
        &self,
        evaluations: &[PolicyEvaluation],
    ) -> brain_policy::PolicyResult<PolicyEvaluation> {
        Ok(PolicyEvaluation {
            passed: evaluations.iter().all(|e| e.passed),
            violated_rules: evaluations
                .iter()
                .flat_map(|e| e.violated_rules.clone())
                .collect(),
            warnings: evaluations
                .iter()
                .flat_map(|e| e.warnings.clone())
                .collect(),
            allowance: evaluations.iter().map(|e| e.allowance).fold(1.0, f64::min),
        })
    }
}

/// A tool registry that returns no candidates.
#[derive(Debug, Default)]
pub struct EmptyToolRegistry;

#[async_trait::async_trait]
impl ToolRegistry for EmptyToolRegistry {
    async fn find_candidates(
        &self,
        _capability: brain_core::ids::ToolCapabilityId,
    ) -> brain_core::BrainResult<Vec<ToolCandidate>> {
        Ok(Vec::new())
    }

    async fn register(&self, _candidate: ToolCandidate) -> brain_core::BrainResult<()> {
        Ok(())
    }

    async fn deregister(&self, _tool_id: ToolId) -> brain_core::BrainResult<()> {
        Ok(())
    }

    async fn list_capabilities(&self) -> brain_core::BrainResult<Vec<ToolCapability>> {
        Ok(Vec::new())
    }
}

/// A reasoning service that returns a fixed, scripted result.
#[derive(Debug)]
pub struct ScriptedReasoningService {
    result: ReasoningResult,
}

impl ScriptedReasoningService {
    pub fn new(result: ReasoningResult) -> Self {
        Self { result }
    }
}

#[async_trait::async_trait]
impl ReasoningService for ScriptedReasoningService {
    async fn reason(&self, _input: &str) -> brain_core::BrainResult<ReasoningResult> {
        Ok(self.result.clone())
    }
}

/// Build a `ReasoningResult` with the given intent and goal description.
pub fn scripted_reasoning(
    intent: &str,
    goal: &str,
    required_capabilities: Vec<&str>,
) -> ReasoningResult {
    ReasoningResult {
        summary: format!("{intent}: {goal}"),
        intent: intent.into(),
        confidence: brain_core::types::Confidence::new(0.9),
        observations: vec![goal.into()],
        entities: Vec::new(),
        inferred_constraints: Vec::new(),
        assumptions: Vec::new(),
        hypotheses: vec![goal.into()],
        selected_hypothesis: Some(goal.into()),
        suggested_goal: goal.into(),
        suggested_priority: brain_core::types::GoalPriority::Normal,
        required_capabilities: required_capabilities
            .into_iter()
            .map(String::from)
            .collect(),
        explanation: "scripted for integration test".into(),
        metadata: Default::default(),
    }
}

/// A scripted agent provider used for multi-agent delegation tests.
#[derive(Debug)]
pub struct ScriptedAgent {
    name: String,
    role: AgentRole,
    capabilities: Vec<AgentCapability>,
    succeed: bool,
}

impl ScriptedAgent {
    pub fn new(name: &str, role: AgentRole, capabilities: Vec<&str>, succeed: bool) -> Self {
        Self {
            name: name.into(),
            role,
            capabilities: capabilities
                .into_iter()
                .map(|c| AgentCapability {
                    name: c.into(),
                    description: format!("provides {c}"),
                    confidence: 0.9,
                })
                .collect(),
            succeed,
        }
    }
}

#[async_trait::async_trait]
impl AgentProvider for ScriptedAgent {
    async fn delegate(
        &self,
        request: DelegationRequest,
    ) -> Result<DelegationResult, brain_core::BrainError> {
        Ok(DelegationResult {
            delegation_id: request.delegation_id,
            agent_id: request.agent_id,
            success: self.succeed,
            outcome: if self.succeed {
                "delegation succeeded".into()
            } else {
                "delegation failed".into()
            },
            artifacts: Vec::new(),
            confidence: 0.9,
            duration_ms: 0,
            error: if self.succeed {
                None
            } else {
                Some("scripted failure".into())
            },
        })
    }

    async fn cancel(&self, _delegation_id: &GoalId) -> Result<(), brain_core::BrainError> {
        Ok(())
    }

    async fn status(
        &self,
        _delegation_id: &GoalId,
    ) -> Result<DelegationStatus, brain_core::BrainError> {
        Ok(DelegationStatus::Completed)
    }

    fn role(&self) -> AgentRole {
        self.role
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn capabilities(&self) -> Vec<AgentCapability> {
        self.capabilities.clone()
    }
}

/// Helper: a `ToolRequirement` for a named capability.
pub fn requirement_for(capability: &str) -> ToolRequirement {
    let mut inputs = std::collections::HashMap::new();
    inputs.insert(
        "__cap".into(),
        brain_core::tool::ToolValue::String(capability.into()),
    );
    ToolRequirement::new(brain_core::ids::ToolCapabilityId::new(), inputs, vec![])
}

/// Helper: an `Arc<dyn GoalStore>` wrapping a fresh in-memory store.
pub fn new_goal_store() -> Arc<dyn brain_goals::store::GoalStore> {
    Arc::new(brain_goals::memory_store::InMemoryGoalStore::new())
}
