use crate::ids::{AgentId, GoalId};
use serde::{Deserialize, Serialize};

/// The role a delegated agent fulfills in the cognitive architecture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AgentRole {
    /// Executes system actions (the Execution Platform)
    Execution,
    /// Provides model-based intelligence (the Intelligence Platform)
    Intelligence,
    /// Observes and interprets environment (the Perception Platform)
    Perception,
    /// Specialized internal reasoning agent
    InternalReasoner,
    /// Specialized research agent
    Researcher,
    /// Specialized coding agent
    Coder,
    /// Remote/plugin agent
    Remote,
}

/// The current status of a delegated goal or task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DelegationStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    TimedOut,
    Revoked,
}

/// Describes a capability an agent provides.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentCapability {
    pub name: String,
    pub description: String,
    pub confidence: f64,
}

/// Registration entry for an available agent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentRegistration {
    pub agent_id: AgentId,
    pub name: String,
    pub role: AgentRole,
    pub capabilities: Vec<AgentCapability>,
    pub is_available: bool,
    pub max_concurrent_tasks: u32,
    pub current_tasks: u32,
}

/// A delegation request sent to an agent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DelegationRequest {
    pub delegation_id: crate::ids::GoalId,
    pub goal_id: GoalId,
    pub agent_id: AgentId,
    pub task_description: String,
    pub expected_outcome: String,
    pub timeout_ms: u64,
    pub priority: u8,
}

/// The result returned by a delegated agent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DelegationResult {
    pub delegation_id: GoalId,
    pub agent_id: AgentId,
    pub success: bool,
    pub outcome: String,
    pub artifacts: Vec<String>,
    pub confidence: f64,
    pub duration_ms: u64,
    pub error: Option<String>,
}

#[async_trait::async_trait]
pub trait AgentProvider: Send + Sync + std::fmt::Debug {
    async fn delegate(
        &self,
        request: DelegationRequest,
    ) -> Result<DelegationResult, crate::BrainError>;
    async fn cancel(&self, delegation_id: &GoalId) -> Result<(), crate::BrainError>;
    async fn status(&self, delegation_id: &GoalId) -> Result<DelegationStatus, crate::BrainError>;
    fn role(&self) -> AgentRole;
    fn name(&self) -> &str;
    fn capabilities(&self) -> Vec<AgentCapability>;
}
