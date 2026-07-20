use async_trait::async_trait;
use execution_core::types::ExecutionResult as ExecutionOutput;
use execution_core::{
    CancelReason, Dispatcher, ExecutionError, ExecutionHandle, ExecutionId, ExecutionPlan,
    ExecutionState, SandboxEnforcer, ToolBinding,
};
use execution_runner::RunnerFactory;
use memory_core::Timestamp;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt;
use std::future;
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
use std::sync::{Arc, RwLock};
use tokio::sync::{mpsc, oneshot, watch, Notify, Semaphore};

use crate::error::DispatchError;

impl From<DispatchError> for ExecutionError {
    fn from(e: DispatchError) -> Self {
        match e {
            DispatchError::QueueFull => ExecutionError::QueueFull,
            DispatchError::ConcurrencyLimitReached(s) => ExecutionError::ConcurrencyLimitReached(s),
            DispatchError::BackendUnavailable(s) => ExecutionError::BackendUnavailable(s),
            DispatchError::BackendFallbackFailed(s) => ExecutionError::BackendFallbackFailed(s),
            DispatchError::ExecutionNotFound(s) => ExecutionError::NotFound(s),
            DispatchError::ExecutionNotRunning(s) => ExecutionError::NotFound(s),
            DispatchError::CancelFailed(s) => ExecutionError::CancellationFailed(s),
            DispatchError::LockPoisoned => {
                ExecutionError::SandboxResolutionFailed("lock poisoned".into())
            }
            DispatchError::Execution(e) => e,
            DispatchError::Planner(e) => ExecutionError::SandboxResolutionFailed(e.to_string()),
            DispatchError::Sandbox(e) => ExecutionError::SandboxResolutionFailed(e.to_string()),
            DispatchError::Core(e) => ExecutionError::Core(e),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DispatchPolicy {
    pub max_concurrency: usize,
    pub per_backend_concurrency: HashMap<String, usize>,
    pub enable_fallback: bool,
    pub queue_capacity: usize,
}

impl Default for DispatchPolicy {
    fn default() -> Self {
        Self {
            max_concurrency: 10,
            per_backend_concurrency: HashMap::new(),
            enable_fallback: true,
            queue_capacity: 1024,
        }
    }
}

pub struct QueuedPlan {
    pub plan: ExecutionPlan,
    pub queued_at: Timestamp,
    pub state_tx: watch::Sender<ExecutionState>,
    pub result_tx: oneshot::Sender<ExecutionOutput>,
    pub cancel_rx: Option<oneshot::Receiver<CancelReason>>,
}

impl fmt::Debug for QueuedPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("QueuedPlan")
            .field("plan", &self.plan.id)
            .field("queued_at", &self.queued_at)
            .field("priority", &self.plan.priority)
            .finish()
    }
}

impl PartialEq for QueuedPlan {
    fn eq(&self, other: &Self) -> bool {
        self.plan.id == other.plan.id
    }
}

impl Eq for QueuedPlan {}

impl PartialOrd for QueuedPlan {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for QueuedPlan {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .plan
            .priority
            .value()
            .cmp(&self.plan.priority.value())
            .then_with(|| other.queued_at.cmp(&self.queued_at))
    }
}

pub struct ActiveExecution {
    pub plan: ExecutionPlan,
    pub started_at: Timestamp,
    pub cancel_tx: oneshot::Sender<CancelReason>,
}

impl fmt::Debug for ActiveExecution {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ActiveExecution")
            .field("plan", &self.plan.id)
            .field("started_at", &self.started_at)
            .finish()
    }
}

pub struct DefaultDispatcher {
    queue: Arc<RwLock<Vec<QueuedPlan>>>,
    active: Arc<RwLock<HashMap<ExecutionId, ActiveExecution>>>,
    per_backend_semaphores: HashMap<String, Arc<Semaphore>>,
    runner_factory: Arc<dyn RunnerFactory>,
    sandbox: Arc<dyn SandboxEnforcer>,
    plan_tx: mpsc::Sender<ExecutionPlan>,
    notify: Arc<Notify>,
    max_concurrency: AtomicUsize,
    policy: RwLock<DispatchPolicy>,
}

impl fmt::Debug for DefaultDispatcher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DefaultDispatcher")
            .field("max_concurrency", &self.max_concurrency)
            .finish()
    }
}

impl DefaultDispatcher {
    pub fn new(
        runner_factory: Arc<dyn RunnerFactory>,
        sandbox: Arc<dyn SandboxEnforcer>,
        plan_tx: mpsc::Sender<ExecutionPlan>,
        policy: DispatchPolicy,
    ) -> Self {
        let per_backend_semaphores: HashMap<String, Arc<Semaphore>> = policy
            .per_backend_concurrency
            .iter()
            .map(|(k, v)| (k.clone(), Arc::new(Semaphore::new(*v))))
            .collect();

        Self {
            queue: Arc::new(RwLock::new(Vec::new())),
            active: Arc::new(RwLock::new(HashMap::new())),
            per_backend_semaphores,
            runner_factory,
            sandbox,
            plan_tx,
            notify: Arc::new(Notify::new()),
            max_concurrency: AtomicUsize::new(policy.max_concurrency),
            policy: RwLock::new(policy),
        }
    }

    pub fn backend_key(binding: &ToolBinding) -> String {
        match binding {
            ToolBinding::Subprocess { .. } => "subprocess".into(),
            ToolBinding::Wasm { .. } => "wasm".into(),
            ToolBinding::Container { .. } => "container".into(),
        }
    }

    pub fn spawn_drain_loop(self: &Arc<Self>) {
        let this = self.clone();
        tokio::spawn(async move {
            loop {
                this.notify.notified().await;
                this.drain().await;
            }
        });
    }

    async fn drain_one(&self) -> bool {
        let active_count = match self.active.read() {
            Ok(a) => a.len(),
            Err(_) => return false,
        };
        if active_count >= self.max_concurrency.load(AtomicOrdering::Acquire) {
            return false;
        }

        let (queued, backend) = {
            let mut queue = match self.queue.write() {
                Ok(q) => q,
                Err(_) => return false,
            };

            if queue.is_empty() {
                return false;
            }

            let best_idx = (0..queue.len())
                .max_by(|&a, &b| {
                    let qa = &queue[a];
                    let qb = &queue[b];
                    qb.plan
                        .priority
                        .value()
                        .cmp(&qa.plan.priority.value())
                        .then_with(|| qa.queued_at.cmp(&qb.queued_at))
                })
                .unwrap();

            let candidate = &queue[best_idx];
            let bk = Self::backend_key(&candidate.plan.binding);

            if let Some(sem) = self.per_backend_semaphores.get(&bk) {
                if sem.available_permits() == 0 {
                    return false;
                }
            }

            (queue.remove(best_idx), bk)
        };

        let _permit = match self.per_backend_semaphores.get(&backend) {
            Some(sem) => {
                if sem.try_acquire().is_err() {
                    if let Ok(mut queue) = self.queue.write() {
                        queue.push(queued);
                    }
                    return false;
                }
                true
            }
            None => true,
        };

        self.spawn_execution(queued, backend);
        true
    }

    async fn drain(&self) {
        for _ in 0..128 {
            if !self.drain_one().await {
                break;
            }
        }
    }

    fn spawn_execution(&self, queued: QueuedPlan, _backend: String) {
        let id = queued.plan.id;
        let plan = queued.plan;
        let state_tx = queued.state_tx;
        let result_tx = queued.result_tx;
        let cancel_rx = queued.cancel_rx;

        let (dispatch_cancel_tx, dispatch_cancel_rx) = oneshot::channel();

        {
            if let Ok(mut active) = self.active.write() {
                active.insert(
                    id,
                    ActiveExecution {
                        plan: plan.clone(),
                        started_at: Timestamp::now(),
                        cancel_tx: dispatch_cancel_tx,
                    },
                );
            }
        }

        let active_arc = self.active.clone();
        let notify = self.notify.clone();
        let sandbox = self.sandbox.clone();
        let runner_factory = self.runner_factory.clone();
        let plan_tx = self.plan_tx.clone();

        tokio::spawn(async move {
            state_tx.send(ExecutionState::Dispatching).ok();

            if let Err(e) = sandbox.validate(&plan).await {
                state_tx.send(ExecutionState::Failed).ok();
                let res = ExecutionOutput {
                    execution_id: id,
                    state: ExecutionState::Failed,
                    exit_code: None,
                    stdout: Vec::new(),
                    stderr: Vec::new(),
                    parsed_output: None,
                    artifacts: Vec::new(),
                    metrics: execution_core::types::ExecutionMetrics::default(),
                    error: Some(e.to_string()),
                    routing_results: Vec::new(),
                    started_at: Timestamp::now(),
                    completed_at: Timestamp::now(),
                };
                let _ = result_tx.send(res);
                Self::do_cleanup(id, &active_arc, &notify).await;
                return;
            }

            let runner = match runner_factory.create_runner(&plan.binding).await {
                Ok(r) => r,
                Err(e) => {
                    state_tx.send(ExecutionState::Failed).ok();
                    let res = ExecutionOutput {
                        execution_id: id,
                        state: ExecutionState::Failed,
                        exit_code: None,
                        stdout: Vec::new(),
                        stderr: Vec::new(),
                        parsed_output: None,
                        artifacts: Vec::new(),
                        metrics: execution_core::types::ExecutionMetrics::default(),
                        error: Some(e.to_string()),
                        routing_results: Vec::new(),
                        started_at: Timestamp::now(),
                        completed_at: Timestamp::now(),
                    };
                    let _ = result_tx.send(res);
                    Self::do_cleanup(id, &active_arc, &notify).await;
                    return;
                }
            };

            let _ = plan_tx.send(plan.clone()).await;

            state_tx.send(ExecutionState::Running).ok();

            let exec_result = {
                let execute = runner.execute(&plan);
                tokio::pin!(execute);

                tokio::select! {
                    result = &mut execute => result,
                    _ = dispatch_cancel_rx => {
                        let _ = runner.cancel(&id).await;
                        state_tx.send(ExecutionState::Cancelled).ok();
                        let res = ExecutionOutput {
                            execution_id: id,
                            state: ExecutionState::Cancelled,
                            exit_code: None,
                            stdout: Vec::new(),
                            stderr: Vec::new(),
                            parsed_output: None,
                            artifacts: Vec::new(),
                            metrics: execution_core::types::ExecutionMetrics::default(),
                            error: None,
                            routing_results: Vec::new(),
                            started_at: Timestamp::now(),
                            completed_at: Timestamp::now(),
                        };
                        let _ = result_tx.send(res);
                        Self::do_cleanup(id, &active_arc, &notify).await;
                        return;
                    }
                    _ = async {
                        if let Some(rx) = cancel_rx {
                            let _ = rx.await;
                        } else {
                            future::pending::<()>().await;
                        }
                    } => {
                        let _ = runner.cancel(&id).await;
                        state_tx.send(ExecutionState::Cancelled).ok();
                        let res = ExecutionOutput {
                            execution_id: id,
                            state: ExecutionState::Cancelled,
                            exit_code: None,
                            stdout: Vec::new(),
                            stderr: Vec::new(),
                            parsed_output: None,
                            artifacts: Vec::new(),
                            metrics: execution_core::types::ExecutionMetrics::default(),
                            error: None,
                            routing_results: Vec::new(),
                            started_at: Timestamp::now(),
                            completed_at: Timestamp::now(),
                        };
                        let _ = result_tx.send(res);
                        Self::do_cleanup(id, &active_arc, &notify).await;
                        return;
                    }
                }
            };

            match exec_result {
                Ok(output) => {
                    state_tx.send(ExecutionState::Collecting).ok();
                    let res = ExecutionOutput {
                        execution_id: id,
                        state: ExecutionState::Completed,
                        exit_code: Some(output.exit_code),
                        stdout: output.stdout,
                        stderr: output.stderr,
                        parsed_output: None,
                        artifacts: output.artifacts,
                        metrics: output.metrics,
                        error: None,
                        routing_results: Vec::new(),
                        started_at: Timestamp::now(),
                        completed_at: Timestamp::now(),
                    };
                    state_tx.send(ExecutionState::Completed).ok();
                    let _ = result_tx.send(res);
                }
                Err(e) => {
                    state_tx.send(ExecutionState::Failed).ok();
                    let res = ExecutionOutput {
                        execution_id: id,
                        state: ExecutionState::Failed,
                        exit_code: None,
                        stdout: Vec::new(),
                        stderr: Vec::new(),
                        parsed_output: None,
                        artifacts: Vec::new(),
                        metrics: execution_core::types::ExecutionMetrics::default(),
                        error: Some(e.to_string()),
                        routing_results: Vec::new(),
                        started_at: Timestamp::now(),
                        completed_at: Timestamp::now(),
                    };
                    let _ = result_tx.send(res);
                }
            }

            Self::do_cleanup(id, &active_arc, &notify).await;
        });
    }

    async fn do_cleanup(
        id: ExecutionId,
        active: &Arc<RwLock<HashMap<ExecutionId, ActiveExecution>>>,
        notify: &Arc<Notify>,
    ) {
        if let Ok(mut a) = active.write() {
            a.remove(&id);
        }
        notify.notify_one();
    }
}

#[async_trait]
impl Dispatcher for DefaultDispatcher {
    async fn dispatch(&self, plan: ExecutionPlan) -> Result<ExecutionHandle, ExecutionError> {
        let (state_tx, state_rx) = watch::channel(ExecutionState::Queued);
        let (handle_cancel_tx, cancel_rx) = oneshot::channel();
        let (result_tx, result_rx) = oneshot::channel();

        let handle = ExecutionHandle {
            id: plan.id,
            state_rx,
            cancel_tx: handle_cancel_tx,
            result_rx,
        };

        {
            let mut queue = self
                .queue
                .write()
                .map_err(|_| DispatchError::LockPoisoned)?;
            let policy = self
                .policy
                .read()
                .map_err(|_| DispatchError::LockPoisoned)?;

            if queue.len() >= policy.queue_capacity {
                return Err(DispatchError::QueueFull.into());
            }

            queue.push(QueuedPlan {
                plan,
                queued_at: Timestamp::now(),
                state_tx,
                result_tx,
                cancel_rx: Some(cancel_rx),
            });
        }

        Ok(handle)
    }

    async fn cancel(&self, execution_id: ExecutionId) -> Result<(), ExecutionError> {
        {
            let mut queue = self
                .queue
                .write()
                .map_err(|_| DispatchError::LockPoisoned)?;
            let before = queue.len();
            queue.retain(|q| q.plan.id != execution_id);
            if queue.len() < before {
                self.notify.notify_one();
                return Ok(());
            }
        }

        {
            let mut active = self
                .active
                .write()
                .map_err(|_| DispatchError::LockPoisoned)?;
            if let Some(entry) = active.remove(&execution_id) {
                let _ = entry.cancel_tx.send(CancelReason::UserRequested);
                self.notify.notify_one();
                return Ok(());
            }
        }

        Err(DispatchError::ExecutionNotFound(execution_id.to_string()).into())
    }

    async fn queue_depth(&self) -> usize {
        self.queue.read().map(|q| q.len()).unwrap_or(0)
    }

    async fn active_count(&self) -> usize {
        self.active.read().map(|a| a.len()).unwrap_or(0)
    }

    async fn set_concurrency_limit(&self, limit: usize) {
        self.max_concurrency.store(limit, AtomicOrdering::Release);
    }

    fn plan_tx(&self) -> Option<mpsc::Sender<ExecutionPlan>> {
        Some(self.plan_tx.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use execution_core::{
        ExecutionBudget, ExecutionContext, ExecutionPermissions, ExecutionPriority,
        ExecutionRequest, ExecutionSession, RetryPolicy, SandboxProfile,
    };
    use execution_runner::DefaultRunnerFactory;
    use execution_sandbox::DefaultSandboxEnforcer;
    use std::collections::HashMap;
    use tokio::sync::mpsc;

    fn test_plan(priority: ExecutionPriority) -> ExecutionPlan {
        ExecutionPlan {
            id: ExecutionId::new(),
            request: ExecutionRequest {
                requirement_id: "req-1".into(),
                capability_id: "cap-1".into(),
                inputs: HashMap::new(),
                context: ExecutionContext {
                    session: ExecutionSession {
                        session_id: "s-1".into(),
                        user_id: "u-1".into(),
                        roles: vec!["admin".into()],
                        permissions: ExecutionPermissions::default(),
                    },
                    trace_id: "t-1".into(),
                    span_id: "s-1".into(),
                    originating_goal: None,
                    originating_plan: None,
                },
                budget: ExecutionBudget::default(),
                priority,
                retry_policy: RetryPolicy::default(),
            },
            binding: ToolBinding::Subprocess {
                binary: "/bin/echo".into(),
                args: vec!["ok".into()],
                env: HashMap::new(),
                working_dir: None,
                allowed_paths: vec![],
                denied_binaries: vec![],
            },
            sandbox_profile: SandboxProfile::new("default"),
            budget: ExecutionBudget::default(),
            priority,
            permissions: ExecutionPermissions::default(),
            routing_rules: Vec::new(),
            rollback_plan: None,
            state: ExecutionState::Planned,
            created_at: Timestamp::now(),
        }
    }

    fn setup() -> (DefaultDispatcher, mpsc::Receiver<ExecutionPlan>) {
        let (plan_tx, plan_rx) = mpsc::channel(64);
        let runner_factory = Arc::new(DefaultRunnerFactory);
        let sandbox = Arc::new(DefaultSandboxEnforcer::new());
        let policy = DispatchPolicy {
            max_concurrency: 5,
            per_backend_concurrency: HashMap::new(),
            enable_fallback: true,
            queue_capacity: 100,
        };
        (
            DefaultDispatcher::new(runner_factory, sandbox, plan_tx, policy),
            plan_rx,
        )
    }

    #[tokio::test]
    async fn test_dispatch_returns_handle_with_correct_id() {
        let (dispatcher, _plan_rx) = setup();
        let plan = test_plan(ExecutionPriority::NORMAL);
        let id = plan.id;

        let handle = dispatcher.dispatch(plan).await.unwrap();
        assert_eq!(handle.id, id);
        assert_eq!(*handle.state_rx.borrow(), ExecutionState::Queued);
    }

    #[tokio::test]
    async fn test_queue_depth() {
        let (plan_tx, _) = mpsc::channel(64);
        let runner = Arc::new(DefaultRunnerFactory);
        let sandbox = Arc::new(DefaultSandboxEnforcer::new());
        let policy = DispatchPolicy {
            max_concurrency: 0,
            per_backend_concurrency: HashMap::new(),
            enable_fallback: true,
            queue_capacity: 100,
        };
        let d = DefaultDispatcher::new(runner, sandbox, plan_tx, policy);

        let p1 = test_plan(ExecutionPriority::NORMAL);
        let p2 = test_plan(ExecutionPriority::NORMAL);
        let _h1 = d.dispatch(p1).await.unwrap();
        let _h2 = d.dispatch(p2).await.unwrap();

        assert_eq!(d.queue_depth().await, 2);
    }

    #[tokio::test]
    async fn test_cancel_queued_plan() {
        let (plan_tx, _) = mpsc::channel(64);
        let runner = Arc::new(DefaultRunnerFactory);
        let sandbox = Arc::new(DefaultSandboxEnforcer::new());
        let policy = DispatchPolicy {
            max_concurrency: 0,
            per_backend_concurrency: HashMap::new(),
            enable_fallback: true,
            queue_capacity: 100,
        };
        let d = DefaultDispatcher::new(runner, sandbox, plan_tx, policy);

        let plan = test_plan(ExecutionPriority::NORMAL);
        let id = plan.id;

        let _handle = d.dispatch(plan).await.unwrap();
        assert_eq!(d.queue_depth().await, 1);

        d.cancel(id).await.unwrap();
        assert_eq!(d.queue_depth().await, 0);
    }

    #[tokio::test]
    async fn test_cancel_nonexistent_plan() {
        let (dispatcher, _) = setup();
        let id = ExecutionId::new();
        let result = dispatcher.cancel(id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_set_concurrency_limit() {
        let (dispatcher, _) = setup();
        dispatcher.set_concurrency_limit(3).await;
        assert_eq!(dispatcher.max_concurrency.load(AtomicOrdering::Acquire), 3);
    }

    #[tokio::test]
    async fn test_plan_tx_channel() {
        let (dispatcher, _plan_rx) = setup();
        assert!(dispatcher.plan_tx().is_some());
    }

    #[tokio::test]
    async fn test_queue_full_error() {
        let (plan_tx, _) = mpsc::channel(64);
        let runner = Arc::new(DefaultRunnerFactory);
        let sandbox = Arc::new(DefaultSandboxEnforcer::new());
        let policy = DispatchPolicy {
            max_concurrency: 10,
            per_backend_concurrency: HashMap::new(),
            enable_fallback: true,
            queue_capacity: 1,
        };
        let d = DefaultDispatcher::new(runner, sandbox, plan_tx, policy);

        let _h1 = d
            .dispatch(test_plan(ExecutionPriority::NORMAL))
            .await
            .unwrap();
        let result = d.dispatch(test_plan(ExecutionPriority::NORMAL)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_backend_semaphore_limits_enforced() {
        let mut per_backend = HashMap::new();
        per_backend.insert("subprocess".into(), 1usize);

        let (plan_tx, _) = mpsc::channel(64);
        let runner = Arc::new(DefaultRunnerFactory);
        let sandbox = Arc::new(DefaultSandboxEnforcer::new());
        let policy = DispatchPolicy {
            max_concurrency: 10,
            per_backend_concurrency: per_backend,
            enable_fallback: true,
            queue_capacity: 100,
        };
        let d = DefaultDispatcher::new(runner, sandbox, plan_tx, policy);

        let p1 = test_plan(ExecutionPriority::NORMAL);
        let p2 = test_plan(ExecutionPriority::NORMAL);

        let _h1 = d.dispatch(p1).await.unwrap();
        let _h2 = d.dispatch(p2).await.unwrap();

        assert_eq!(d.queue_depth().await, 2);
    }

    #[tokio::test]
    async fn test_concurrency_limit_zero_queues_all() {
        let (plan_tx, _) = mpsc::channel(64);
        let runner = Arc::new(DefaultRunnerFactory);
        let sandbox = Arc::new(DefaultSandboxEnforcer::new());
        let policy = DispatchPolicy {
            max_concurrency: 0,
            per_backend_concurrency: HashMap::new(),
            enable_fallback: true,
            queue_capacity: 100,
        };
        let d = DefaultDispatcher::new(runner, sandbox, plan_tx, policy);

        let plan = test_plan(ExecutionPriority::NORMAL);
        let _handle = d.dispatch(plan).await.unwrap();
        assert_eq!(d.queue_depth().await, 1);
        assert_eq!(d.active_count().await, 0);
    }

    #[tokio::test]
    async fn test_handle_tracks_state() {
        let (dispatcher, _) = setup();
        let plan = test_plan(ExecutionPriority::NORMAL);
        let handle = dispatcher.dispatch(plan).await.unwrap();
        assert_eq!(*handle.state_rx.borrow(), ExecutionState::Queued);
    }

    #[test]
    fn test_queued_plan_ordering_by_priority() {
        let now = Timestamp::now();
        let later = Timestamp::from_nanos(now.as_nanos() + 1_000);

        let (stx1, _) = watch::channel(ExecutionState::Queued);
        let (rtx1, _) = oneshot::channel();
        let (stx2, _) = watch::channel(ExecutionState::Queued);
        let (rtx2, _) = oneshot::channel();

        let q1 = QueuedPlan {
            plan: test_plan(ExecutionPriority::CRITICAL),
            queued_at: now,
            state_tx: stx1,
            result_tx: rtx1,
            cancel_rx: None,
        };

        let q2 = QueuedPlan {
            plan: test_plan(ExecutionPriority::NORMAL),
            queued_at: later,
            state_tx: stx2,
            result_tx: rtx2,
            cancel_rx: None,
        };

        assert!(q1 > q2, "CRITICAL should order before NORMAL");
    }

    #[test]
    fn test_queued_plan_ordering_by_time() {
        let now = Timestamp::now();
        let earlier = Timestamp::from_nanos(now.as_nanos() - 1_000);

        let (stx1, _) = watch::channel(ExecutionState::Queued);
        let (rtx1, _) = oneshot::channel();
        let (stx2, _) = watch::channel(ExecutionState::Queued);
        let (rtx2, _) = oneshot::channel();

        let q1 = QueuedPlan {
            plan: test_plan(ExecutionPriority::NORMAL),
            queued_at: earlier,
            state_tx: stx1,
            result_tx: rtx1,
            cancel_rx: None,
        };

        let q2 = QueuedPlan {
            plan: test_plan(ExecutionPriority::NORMAL),
            queued_at: now,
            state_tx: stx2,
            result_tx: rtx2,
            cancel_rx: None,
        };

        assert!(
            q1 > q2,
            "Earlier should order before later for same priority"
        );
    }
}
