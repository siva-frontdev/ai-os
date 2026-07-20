use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Debug;
use tokio::sync::mpsc;

use crate::types::*;

// ── ToolRegistry ───────────────────────────────────────────

#[async_trait]
pub trait ToolRegistry: Debug + Send + Sync {
    async fn register(
        &self,
        binding: ToolBinding,
        capabilities: Vec<ExecutionCapability>,
    ) -> crate::error::ExecutionResult<()>;

    async fn unregister(&self, tool_id: &str) -> crate::error::ExecutionResult<()>;

    async fn resolve(&self, capability_id: &str)
        -> crate::error::ExecutionResult<Vec<ToolBinding>>;

    async fn list_capabilities(&self) -> crate::error::ExecutionResult<Vec<ExecutionCapability>>;

    async fn find_by_name(&self, name: &str) -> crate::error::ExecutionResult<Option<ToolBinding>>;
}

// ── ToolResolver ───────────────────────────────────────────

#[async_trait]
pub trait ToolResolver: Debug + Send + Sync {
    async fn resolve_capability(
        &self,
        capability_id: &str,
    ) -> crate::error::ExecutionResult<ToolBinding>;

    async fn resolve_with_fallback(
        &self,
        capability_id: &str,
        preferred_backends: &[String],
    ) -> crate::error::ExecutionResult<ToolBinding>;

    async fn check_capability(&self, capability_id: &str, tool: &ToolBinding) -> bool;
}

// ── ExecutionPlanner ───────────────────────────────────────

#[async_trait]
pub trait ExecutionPlanner: Debug + Send + Sync {
    async fn plan(&self, request: ExecutionRequest)
        -> crate::error::ExecutionResult<ExecutionPlan>;

    async fn plan_batch(
        &self,
        requests: Vec<ExecutionRequest>,
    ) -> crate::error::ExecutionResult<Vec<ExecutionPlan>>;

    async fn expand_requirements(
        &self,
        request: ExecutionRequest,
    ) -> crate::error::ExecutionResult<Vec<ExecutionRequest>>;

    async fn build_dependency_graph(
        &self,
        plans: &[ExecutionPlan],
    ) -> crate::error::ExecutionResult<DependencyGraph>;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DependencyGraph {
    pub nodes: Vec<ExecutionPlan>,
    pub edges: Vec<DependencyEdge>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DependencyEdge {
    pub from_id: ExecutionId,
    pub to_id: ExecutionId,
    pub edge_type: DependencyType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DependencyType {
    Blocking,
    DataFlow,
    Ordering,
}

// ── Dispatcher ─────────────────────────────────────────────

#[async_trait]
pub trait Dispatcher: Debug + Send + Sync {
    async fn dispatch(&self, plan: ExecutionPlan)
        -> crate::error::ExecutionResult<ExecutionHandle>;

    async fn cancel(&self, execution_id: ExecutionId) -> crate::error::ExecutionResult<()>;

    async fn queue_depth(&self) -> usize;

    async fn active_count(&self) -> usize;

    async fn set_concurrency_limit(&self, limit: usize);

    fn plan_tx(&self) -> Option<mpsc::Sender<ExecutionPlan>>;
}

// ── Runner ─────────────────────────────────────────────────

#[async_trait]
pub trait Runner: Debug + Send + Sync {
    async fn execute(&self, plan: &ExecutionPlan) -> crate::error::ExecutionResult<RunnerOutput>;

    async fn cancel(&self, execution_id: &ExecutionId) -> crate::error::ExecutionResult<()>;

    async fn health(&self) -> crate::error::ExecutionResult<()>;

    fn backend_type(&self) -> &'static str;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunnerOutput {
    pub execution_id: ExecutionId,
    pub exit_code: i32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub metrics: ExecutionMetrics,
    pub artifacts: Vec<ExecutionArtifact>,
}

// ── SandboxEnforcer ────────────────────────────────────────

#[async_trait]
pub trait SandboxEnforcer: Debug + Send + Sync {
    async fn resolve_profile(
        &self,
        profile_name: &str,
    ) -> crate::error::ExecutionResult<SandboxProfile>;

    async fn validate(&self, plan: &ExecutionPlan) -> crate::error::ExecutionResult<()>;

    async fn check_permissions(
        &self,
        binding: &ToolBinding,
        profile: &SandboxProfile,
    ) -> crate::error::ExecutionResult<()>;

    async fn check_capabilities(
        &self,
        binding: &ToolBinding,
        required: &[String],
    ) -> crate::error::ExecutionResult<()>;

    fn register_profile(&self, profile: SandboxProfile);
}

// ── ExecutionMonitor ───────────────────────────────────────

#[async_trait]
pub trait ExecutionMonitor: Debug + Send + Sync {
    async fn start_watch(
        &self,
        execution_id: ExecutionId,
        budget: ExecutionBudget,
        cancel_tx: tokio::sync::oneshot::Sender<CancelReason>,
    );

    async fn stop_watch(&self, execution_id: &ExecutionId);

    async fn report_heartbeat(&self, execution_id: &ExecutionId, metrics: ExecutionMetrics);

    async fn report_progress(&self, execution_id: &ExecutionId, progress: f64, message: String);

    async fn current_metrics(
        &self,
        execution_id: &ExecutionId,
    ) -> crate::error::ExecutionResult<Option<ExecutionMetrics>>;

    async fn health(&self) -> crate::error::ExecutionResult<()>;
}

// ── ResultCollector ────────────────────────────────────────

#[async_trait]
pub trait ResultCollector: Debug + Send + Sync {
    async fn collect(&self, output: RunnerOutput)
        -> crate::error::ExecutionResult<ExecutionResult>;

    async fn collect_with_parse(
        &self,
        output: RunnerOutput,
        parser: &dyn OutputParser,
    ) -> crate::error::ExecutionResult<ExecutionResult>;

    async fn store_artifact(
        &self,
        execution_id: &ExecutionId,
        artifact: ExecutionArtifact,
    ) -> crate::error::ExecutionResult<()>;
}

// ── OutputParser ───────────────────────────────────────────

#[async_trait]
pub trait OutputParser: Debug + Send + Sync {
    async fn parse(&self, data: &[u8]) -> crate::error::ExecutionResult<Option<serde_json::Value>>;

    fn name(&self) -> &'static str;

    fn mime_types(&self) -> Vec<&'static str>;
}

// ── OutputRouter ───────────────────────────────────────────

#[async_trait]
pub trait OutputRouter: Debug + Send + Sync {
    async fn route(
        &self,
        result: &ExecutionResult,
    ) -> crate::error::ExecutionResult<Vec<RouteResult>>;

    async fn route_to(
        &self,
        result: &ExecutionResult,
        destination: &RouteDestination,
    ) -> crate::error::ExecutionResult<RouteResult>;

    fn add_rule(&self, rule: RoutingRule);
}

// ── RecoveryManager ────────────────────────────────────────

#[async_trait]
pub trait RecoveryManager: Debug + Send + Sync {
    async fn handle_failure(
        &self,
        result: &ExecutionResult,
        plan: &ExecutionPlan,
    ) -> crate::error::ExecutionResult<RecoveryAction>;

    async fn handle_timeout(
        &self,
        result: &ExecutionResult,
        plan: &ExecutionPlan,
    ) -> crate::error::ExecutionResult<RecoveryAction>;

    async fn handle_cancellation(
        &self,
        result: &ExecutionResult,
        plan: &ExecutionPlan,
    ) -> crate::error::ExecutionResult<RecoveryAction>;

    async fn execute_rollback(
        &self,
        plan: &ExecutionPlan,
        result: &ExecutionResult,
    ) -> crate::error::ExecutionResult<()>;

    async fn retry(&self, plan: &ExecutionPlan) -> crate::error::ExecutionResult<ExecutionPlan>;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RecoveryAction {
    Retry {
        retry_plan: ExecutionPlan,
        delay_ms: u64,
    },
    Rollback,
    Escalate {
        reason: String,
        context: String,
    },
    Ignore,
}

// ── PipelineManager ────────────────────────────────────────

#[async_trait]
pub trait PipelineManager: Debug + Send + Sync {
    async fn submit(
        &self,
        request: ExecutionRequest,
    ) -> crate::error::ExecutionResult<ExecutionHandle>;

    async fn cancel(&self, execution_id: ExecutionId) -> crate::error::ExecutionResult<()>;

    async fn get_status(
        &self,
        execution_id: &ExecutionId,
    ) -> crate::error::ExecutionResult<ExecutionState>;

    async fn get_result(
        &self,
        execution_id: &ExecutionId,
    ) -> crate::error::ExecutionResult<Option<ExecutionResult>>;

    async fn health(&self) -> crate::error::ExecutionResult<()>;

    fn stats(&self) -> PipelineStats;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PipelineStats {
    pub total_submitted: u64,
    pub total_completed: u64,
    pub total_failed: u64,
    pub currently_active: usize,
    pub queue_depth: usize,
    pub stages: HashMap<String, StageStats>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StageStats {
    pub processed: u64,
    pub failed: u64,
    pub bypassed: bool,
}

// ── ExecutionCoordinator ───────────────────────────────────

#[async_trait]
pub trait ExecutionCoordinator: Debug + Send + Sync {
    async fn submit(
        &self,
        request: ExecutionRequest,
    ) -> crate::error::ExecutionResult<ExecutionHandle>;

    async fn cancel(&self, execution_id: ExecutionId) -> crate::error::ExecutionResult<()>;

    async fn get_status(
        &self,
        execution_id: &ExecutionId,
    ) -> crate::error::ExecutionResult<ExecutionState>;

    async fn get_result(
        &self,
        execution_id: &ExecutionId,
    ) -> crate::error::ExecutionResult<Option<ExecutionResult>>;

    async fn health(&self) -> crate::error::ExecutionResult<()>;

    async fn pipeline_stats(&self) -> PipelineStats;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dependency_graph_defaults() {
        let graph = DependencyGraph {
            nodes: Vec::new(),
            edges: Vec::new(),
        };
        assert!(graph.nodes.is_empty());
        assert!(graph.edges.is_empty());
    }

    #[test]
    fn test_pipeline_stats_defaults() {
        let stats = PipelineStats {
            total_submitted: 0,
            total_completed: 0,
            total_failed: 0,
            currently_active: 0,
            queue_depth: 0,
            stages: HashMap::new(),
        };
        assert_eq!(stats.total_submitted, 0);
    }

    #[test]
    fn test_recovery_action_retry() {
        let plan = ExecutionPlan {
            id: ExecutionId::new(),
            request: ExecutionRequest {
                requirement_id: "r1".into(),
                capability_id: "c1".into(),
                inputs: HashMap::new(),
                context: ExecutionContext {
                    session: ExecutionSession {
                        session_id: "s1".into(),
                        user_id: "u1".into(),
                        roles: Vec::new(),
                        permissions: ExecutionPermissions::default(),
                    },
                    trace_id: "t1".into(),
                    span_id: "s1".into(),
                    originating_goal: None,
                    originating_plan: None,
                },
                budget: ExecutionBudget::default(),
                priority: ExecutionPriority::default(),
                retry_policy: RetryPolicy::default(),
            },
            binding: ToolBinding::Subprocess {
                binary: "/bin/true".into(),
                args: Vec::new(),
                env: HashMap::new(),
                working_dir: None,
                allowed_paths: Vec::new(),
                denied_binaries: Vec::new(),
            },
            sandbox_profile: SandboxProfile::new("default"),
            budget: ExecutionBudget::default(),
            priority: ExecutionPriority::default(),
            permissions: ExecutionPermissions::default(),
            routing_rules: Vec::new(),
            rollback_plan: None,
            state: ExecutionState::Failed,
            created_at: memory_core::Timestamp::now(),
        };

        let action = RecoveryAction::Retry {
            retry_plan: plan,
            delay_ms: 1000,
        };
        match action {
            RecoveryAction::Retry { delay_ms, .. } => assert_eq!(delay_ms, 1000),
            _ => panic!("wrong variant"),
        }
    }
}
