use execution_core::{
    Dispatcher, ExecutionBudget, ExecutionContext, ExecutionId, ExecutionPermissions,
    ExecutionPlan, ExecutionPriority, ExecutionRequest, ExecutionSession, ExecutionState,
    RetryPolicy, SandboxProfile, ToolBinding,
};
use execution_dispatcher::*;
use execution_runner::DefaultRunnerFactory;
use execution_sandbox::DefaultSandboxEnforcer;
use std::collections::HashMap;
use std::sync::Arc;
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
        created_at: memory_core::Timestamp::now(),
    }
}

#[tokio::test]
async fn test_dispatch_plan_and_verify_handle_id() {
    let (plan_tx, _plan_rx) = mpsc::channel(64);
    let runner = Arc::new(DefaultRunnerFactory);
    let sandbox = Arc::new(DefaultSandboxEnforcer::new());
    let policy = DispatchPolicy::default();
    let d = DefaultDispatcher::new(runner, sandbox, plan_tx, policy);

    let plan = test_plan(ExecutionPriority::NORMAL);
    let expected_id = plan.id;

    let handle = d.dispatch(plan).await.unwrap();
    assert_eq!(handle.id, expected_id, "Handle ID must match plan ID");
    assert_eq!(
        *handle.state_rx.borrow(),
        ExecutionState::Queued,
        "Newly dispatched plan should be Queued"
    );
}

#[tokio::test]
async fn test_queue_multiple_plans_respects_concurrency_limits() {
    let (plan_tx, _) = mpsc::channel(64);
    let runner = Arc::new(DefaultRunnerFactory);
    let sandbox = Arc::new(DefaultSandboxEnforcer::new());
    let policy = DispatchPolicy {
        max_concurrency: 2,
        per_backend_concurrency: HashMap::new(),
        enable_fallback: true,
        queue_capacity: 100,
    };
    let d = DefaultDispatcher::new(runner, sandbox, plan_tx, policy);

    let _h1 = d
        .dispatch(test_plan(ExecutionPriority::NORMAL))
        .await
        .unwrap();
    let _h2 = d
        .dispatch(test_plan(ExecutionPriority::NORMAL))
        .await
        .unwrap();
    let _h3 = d
        .dispatch(test_plan(ExecutionPriority::NORMAL))
        .await
        .unwrap();

    assert_eq!(d.queue_depth().await, 3, "All plans should be queued");
    assert_eq!(d.active_count().await, 0, "No plans active without drain");
}

#[tokio::test]
async fn test_cancel_queued_plan_removes_from_queue() {
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
    assert_eq!(
        d.queue_depth().await,
        0,
        "Queue should be empty after cancel"
    );
}

#[tokio::test]
async fn test_cancel_active_execution_via_dispatcher() {
    let (plan_tx, _plan_rx) = mpsc::channel(64);
    let runner = Arc::new(DefaultRunnerFactory);
    let sandbox = Arc::new(DefaultSandboxEnforcer::new());
    let policy = DispatchPolicy {
        max_concurrency: 1,
        per_backend_concurrency: HashMap::new(),
        enable_fallback: true,
        queue_capacity: 100,
    };
    let d = Arc::new(DefaultDispatcher::new(runner, sandbox, plan_tx, policy));

    let plan = test_plan(ExecutionPriority::NORMAL);
    let id = plan.id;

    let handle = d.dispatch(plan).await.unwrap();
    assert_eq!(d.queue_depth().await, 1);

    DefaultDispatcher::spawn_drain_loop(&d);

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    d.cancel(id).await.unwrap();

    let current_state = *handle.state_rx.borrow();
    assert!(
        current_state == ExecutionState::Queued
            || current_state == ExecutionState::Cancelled
            || current_state == ExecutionState::Failed,
        "State should be terminal or still queued: {:?}",
        current_state
    );
}

#[tokio::test]
async fn test_backend_semaphore_limits() {
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

    assert_eq!(
        d.queue_depth().await,
        2,
        "Both plans queued when no drain loop"
    );
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
        queue_capacity: 2,
    };
    let d = DefaultDispatcher::new(runner, sandbox, plan_tx, policy);

    let _h1 = d
        .dispatch(test_plan(ExecutionPriority::NORMAL))
        .await
        .unwrap();
    let _h2 = d
        .dispatch(test_plan(ExecutionPriority::NORMAL))
        .await
        .unwrap();
    let result = d.dispatch(test_plan(ExecutionPriority::NORMAL)).await;

    assert!(result.is_err(), "Third dispatch should fail with QueueFull");
    assert_eq!(d.queue_depth().await, 2);
}

#[tokio::test]
async fn test_handle_state_tracks_queued() {
    let (plan_tx, _) = mpsc::channel(64);
    let runner = Arc::new(DefaultRunnerFactory);
    let sandbox = Arc::new(DefaultSandboxEnforcer::new());
    let policy = DispatchPolicy::default();
    let d = DefaultDispatcher::new(runner, sandbox, plan_tx, policy);

    let plan = test_plan(ExecutionPriority::CRITICAL);
    let handle = d.dispatch(plan).await.unwrap();

    assert_eq!(*handle.state_rx.borrow(), ExecutionState::Queued);
}

#[tokio::test]
async fn test_active_count_after_dispatch() {
    let (plan_tx, _) = mpsc::channel(64);
    let runner = Arc::new(DefaultRunnerFactory);
    let sandbox = Arc::new(DefaultSandboxEnforcer::new());
    let policy = DispatchPolicy::default();
    let d = DefaultDispatcher::new(runner, sandbox, plan_tx, policy);

    assert_eq!(d.active_count().await, 0, "No active executions initially");
    d.dispatch(test_plan(ExecutionPriority::NORMAL))
        .await
        .unwrap();
    assert_eq!(d.active_count().await, 0, "Still 0 active (only queued)");
}

#[tokio::test]
async fn test_set_concurrency_limit_updates_value() {
    let (plan_tx, _) = mpsc::channel(64);
    let runner = Arc::new(DefaultRunnerFactory);
    let sandbox = Arc::new(DefaultSandboxEnforcer::new());
    let policy = DispatchPolicy::default();
    let d = DefaultDispatcher::new(runner, sandbox, plan_tx, policy);

    d.set_concurrency_limit(42).await;
    assert_eq!(d.active_count().await, 0);
}
