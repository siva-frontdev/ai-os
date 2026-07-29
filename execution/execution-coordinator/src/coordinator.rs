use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

use async_trait::async_trait;
use memory_core::Timestamp;

pub use execution_core::error::ExecutionResult;
use execution_core::event::ExecutionEvent;
use execution_core::traits::{
    ExecutionCoordinator, ExecutionMonitor, ExecutionPlanner, OutputRouter, PipelineManager,
    PipelineStats, RecoveryManager, ResolutionPipeline, ResultCollector, SandboxEnforcer,
    StageStats, ToolRegistry,
};
use execution_core::types::{
    ExecutionBudget, ExecutionHandle, ExecutionId, ExecutionPermissions, ExecutionPlan,
    ExecutionPriority as CorePriority, ExecutionRequest, ExecutionSession, ExecutionState,
    RetryPolicy, RouteDestination, RoutingRule, SandboxProfile,
};

use crate::error::CoordinatorResult;
use crate::recovery_adapter::DefaultCoordinatorRecoveryManager;

/// Wires all pipeline stages together into a single orchestrator.
#[derive(Debug)]
pub struct DefaultCoordinator {
    registry: Arc<dyn ToolRegistry>,
    planner: Arc<dyn ExecutionPlanner>,
    sandbox: Arc<dyn SandboxEnforcer>,
    dispatcher: Arc<dyn execution_core::traits::Dispatcher>,
    monitor: Arc<dyn ExecutionMonitor>,
    collector: Arc<dyn ResultCollector>,
    router: Arc<dyn OutputRouter>,
    recovery: Arc<dyn RecoveryManager>,
    resolution_pipeline: Option<Arc<dyn ResolutionPipeline>>,
    plan_tx: Option<mpsc::Sender<ExecutionPlan>>,
    event_bus_tx: Option<mpsc::Sender<ExecutionEvent>>,
    running: AtomicBool,
    stats: RwLock<PipelineStats>,
}

impl DefaultCoordinator {
    /// Create a new coordinator with fully wired stages.
    pub fn new(
        registry: Arc<dyn ToolRegistry>,
        planner: Arc<dyn ExecutionPlanner>,
        sandbox: Arc<dyn SandboxEnforcer>,
        dispatcher: Arc<dyn execution_core::traits::Dispatcher>,
        monitor: Arc<dyn ExecutionMonitor>,
        collector: Arc<dyn ResultCollector>,
        router: Arc<dyn OutputRouter>,
        recovery: Arc<dyn RecoveryManager>,
    ) -> Self {
        Self {
            registry,
            planner,
            sandbox,
            dispatcher,
            monitor,
            collector,
            router,
            recovery,
            resolution_pipeline: None,
            plan_tx: None,
            event_bus_tx: None,
            running: AtomicBool::new(false),
            stats: RwLock::new(PipelineStats {
                total_submitted: 0,
                total_completed: 0,
                total_failed: 0,
                currently_active: 0,
                queue_depth: 0,
                stages: HashMap::new(),
            }),
        }
    }

    /// Attach a channel for observability.
    pub fn with_plan_tx(mut self, tx: mpsc::Sender<ExecutionPlan>) -> Self {
        self.plan_tx = Some(tx);
        self
    }

    /// Attach an event bus publisher.
    pub fn with_event_bus(mut self, tx: mpsc::Sender<ExecutionEvent>) -> Self {
        self.event_bus_tx = Some(tx);
        self
    }

    /// Attach the autonomous capability resolution pipeline.
    /// When set, the coordinator will use the resolution pipeline to
    /// discover, synthesize, and resolve capabilities before falling
    /// back to the standard planner.
    pub fn with_resolution_pipeline(mut self, pipeline: Arc<dyn ResolutionPipeline>) -> Self {
        self.resolution_pipeline = Some(pipeline);
        self
    }

    /// Access the resolution pipeline if configured.
    pub fn resolution_pipeline(&self) -> Option<&Arc<dyn ResolutionPipeline>> {
        self.resolution_pipeline.as_ref()
    }

    /// Whether the coordinator is currently running.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }

    fn emit_event(&self, ev: ExecutionEvent) {
        if let Some(ref tx) = self.event_bus_tx {
            let _ = tx.try_send(ev);
        }
    }

    fn bump_submitted(&self) {
        if let Ok(mut s) = self.stats.try_write() {
            s.total_submitted += 1;
        }
    }
}

impl DefaultCoordinator {
    /// Start the coordinator.
    pub fn start(&self) -> CoordinatorResult<()> {
        use crate::CoordinatorError;
        if self.running.swap(true, Ordering::AcqRel) {
            return Err(CoordinatorError::LifecycleConflict(
                "already running".into(),
            ));
        }
        self.emit_event(ExecutionEvent::CoordinatorReady(
            execution_core::event::CoordinatorReady {
                stages: vec![
                    "planner".into(),
                    "dispatcher".into(),
                    "monitor".into(),
                    "collector".into(),
                    "router".into(),
                    "recovery".into(),
                ],
                timestamp: Timestamp::now(),
            },
        ));
        Ok(())
    }

    /// Stop the coordinator.
    pub async fn stop(&self) -> CoordinatorResult<()> {
        use crate::CoordinatorError;
        if !self.running.swap(false, Ordering::AcqRel) {
            return Ok(());
        }
        Ok(())
    }
}

#[async_trait]
impl ExecutionCoordinator for DefaultCoordinator {
    async fn submit(&self, request: ExecutionRequest) -> ExecutionResult<ExecutionHandle> {
        self.bump_submitted();

        let plan = if let Some(ref pipeline) = self.resolution_pipeline {
            let plan = pipeline.resolve(request.clone()).await?;

            let handle = self.dispatcher.dispatch(plan.clone()).await?;

            self.emit_event(ExecutionEvent::PlanCreated(
                execution_core::event::ExecutionPlanCreated {
                    plan_id: plan.id,
                    requirement_id: plan.request.requirement_id.clone(),
                    candidate_name: "resolution_pipeline".into(),
                    estimated_duration_ms: plan.budget.timeout_ms,
                    timestamp: Timestamp::now(),
                },
            ));

            if let Some(ref tx) = self.plan_tx {
                let _ = tx.try_send(plan.clone());
            }

            return Ok(handle);
        } else {
            self.planner.plan(request.clone()).await?
        };

        self.emit_event(ExecutionEvent::PlanCreated(
            execution_core::event::ExecutionPlanCreated {
                plan_id: plan.id,
                requirement_id: plan.request.requirement_id.clone(),
                candidate_name: "resolved".into(),
                estimated_duration_ms: plan.budget.timeout_ms,
                timestamp: Timestamp::now(),
            },
        ));

        if let Some(ref tx) = self.plan_tx {
            let _ = tx.try_send(plan.clone());
        }

        self.dispatcher.dispatch(plan).await
    }

    async fn cancel(&self, id: ExecutionId) -> ExecutionResult<()> {
        self.dispatcher.cancel(id).await?;
        self.emit_event(ExecutionEvent::Cancelled(
            execution_core::event::ExecutionCancelled {
                execution_id: id,
                reason: "coordinator_cancel".into(),
                partial_output: true,
                timestamp: Timestamp::now(),
            },
        ));
        Ok(())
    }

    async fn get_status(&self, _id: &ExecutionId) -> ExecutionResult<ExecutionState> {
        Ok(ExecutionState::Queued)
    }

    async fn get_result(
        &self,
        _id: &ExecutionId,
    ) -> ExecutionResult<Option<execution_core::types::ExecutionResult>> {
        Ok(None)
    }

    async fn health(&self) -> ExecutionResult<()> {
        self.monitor.health().await?;
        let _ = self.dispatcher.queue_depth().await;
        Ok(())
    }

    async fn pipeline_stats(&self) -> PipelineStats {
        self.stats.read().await.clone()
    }
}

#[async_trait]
impl PipelineManager for DefaultCoordinator {
    async fn submit(&self, request: ExecutionRequest) -> ExecutionResult<ExecutionHandle> {
        ExecutionCoordinator::submit(self, request).await
    }

    async fn cancel(&self, id: ExecutionId) -> ExecutionResult<()> {
        ExecutionCoordinator::cancel(self, id).await
    }

    async fn get_status(&self, id: &ExecutionId) -> ExecutionResult<ExecutionState> {
        ExecutionCoordinator::get_status(self, id).await
    }

    async fn get_result(
        &self,
        id: &ExecutionId,
    ) -> ExecutionResult<Option<execution_core::types::ExecutionResult>> {
        ExecutionCoordinator::get_result(self, id).await
    }

    async fn health(&self) -> ExecutionResult<()> {
        ExecutionCoordinator::health(self).await
    }

    fn stats(&self) -> PipelineStats {
        self.stats
            .try_read()
            .map(|s| s.clone())
            .unwrap_or_else(|_| PipelineStats {
                total_submitted: 0,
                total_completed: 0,
                total_failed: 0,
                currently_active: 0,
                queue_depth: 0,
                stages: HashMap::new(),
            })
    }
}
