//! Integration tests for the Runtime Platform.
//!
//! Exercises the full lifecycle: create runtime → register with
//! Core → start → create tasks → schedule → supervise → stop.
use std::collections::HashMap;
use std::sync::Arc;

use ai_os_core::bootstrap::PlatformBuilder;
use ai_os_core::error::CoreError;
use ai_os_core::events::{typed_subscribe, EventHandler};
use ai_os_core::lifecycle::Service;
use async_trait::async_trait;

use ai_os_runtime::context::ContextManager;
use ai_os_runtime::permission::{self, Permission, PermissionChecker};
use ai_os_runtime::resource::ResourceUsage;
use ai_os_runtime::scheduler::Scheduler;
use ai_os_runtime::session::{SessionId, SessionManager};
use ai_os_runtime::state::{RuntimePhase, RuntimeStateMachine};
use ai_os_runtime::supervisor::{RestartPolicy, Supervisor};
use ai_os_runtime::task::{Priority, TaskManager};
use ai_os_runtime::Runtime;

// ── Helper: bootstrap Core + Runtime ─────────────────────

async fn setup() -> (ai_os_core::application::Application, Runtime) {
    let app = PlatformBuilder::new()
        .without_console_sink()
        .build()
        .await
        .expect("bootstrap");

    let rt = Runtime::new(app.event_bus().clone(), app.logger().clone());
    (app, rt)
}

// ── Integration tests ────────────────────────────────────

#[tokio::test]
async fn full_integration_session_to_task() {
    let (app, rt) = setup().await;

    // 1. Create a session
    let session = rt.session_manager.create_session(HashMap::new());
    assert_eq!(rt.session_manager.session_count(), 1);

    // 2. Check permissions
    assert!(rt
        .permission_checker
        .check(&session.permission_ctx, &[Permission::Read])
        .is_ok());
    assert!(rt
        .permission_checker
        .check(&session.permission_ctx, &[Permission::Admin])
        .is_err());

    // 3. Create a task
    let task =
        rt.task_manager
            .create_task(Some(session.id.clone()), Priority::NORMAL, HashMap::new());
    assert_eq!(rt.task_manager.task_count(), 1);
    assert_eq!(task.state, ai_os_runtime::task::TaskState::Pending);

    // 4. Enqueue in scheduler
    rt.scheduler
        .enqueue(ai_os_runtime::task::TaskHandle {
            task_id: task.id.clone(),
            priority: Priority::NORMAL,
            session_id: Some(session.id.clone()),
            created_at: task.created_at,
        })
        .unwrap();
    assert_eq!(rt.scheduler.queue_depth(), 1);

    // 5. Dequeue (simulate worker)
    let dequeued = rt.scheduler.dequeue().unwrap();
    assert_eq!(dequeued.task_id, task.id);

    // 6. Mark as running → completed
    rt.task_manager
        .update_state(&task.id, ai_os_runtime::task::TaskState::Running)
        .unwrap();
    rt.task_manager
        .update_state(&task.id, ai_os_runtime::task::TaskState::Completed)
        .unwrap();
    assert!(rt
        .task_manager
        .get_task(&task.id)
        .unwrap()
        .state
        .is_terminal());

    // 7. Track resources
    rt.resource_manager
        .track(
            &task.id,
            ResourceUsage {
                cpu_percent: 5.0,
                memory_bytes: 2048,
            },
        )
        .unwrap();
    assert!((rt.resource_manager.total_usage().cpu_percent - 5.0).abs() < 1e-6);

    // 8. Clean up session
    rt.session_manager.destroy_session(&session.id).unwrap();
    assert_eq!(rt.session_manager.session_count(), 0);
}

#[tokio::test]
async fn supervisor_restarts_on_failure() {
    let (app, rt) = setup().await;

    let task = rt
        .task_manager
        .create_task(None, Priority::NORMAL, HashMap::new());

    // Supervise with OnFailure policy
    rt.supervisor
        .supervise(task.id.clone(), RestartPolicy::OnFailure { max_retries: 2 })
        .unwrap();

    // Record a failure
    let should_restart = rt
        .supervisor
        .record_failure(&task.id, "connection timeout")
        .unwrap();
    assert!(should_restart);

    let status = rt.supervisor.status(&task.id).unwrap();
    assert_eq!(status.retries, 1);
    assert!(status.last_failure_reason.unwrap().contains("timeout"));

    // Second failure: still should restart
    assert!(rt.supervisor.record_failure(&task.id, "again").unwrap());

    // Third failure: max retries exhausted
    assert!(!rt.supervisor.record_failure(&task.id, "done").unwrap());
}

#[tokio::test]
async fn context_propagation() {
    let (app, rt) = setup().await;

    let root = rt.context_manager.current();
    let child = rt.context_manager.create_child(&root);

    assert_eq!(child.trace_id, root.trace_id);
    assert_ne!(child.span_id, root.span_id);
    assert_eq!(child.parent_span_id, Some(root.span_id.clone()));
}

#[tokio::test]
async fn resource_limits_are_enforced() {
    let (app, rt) = setup().await;

    rt.resource_manager
        .set_limits(ai_os_runtime::resource::ResourceLimits {
            max_cpu_percent: 50.0,
            max_memory_bytes: 1_000_000,
            max_tasks: 10,
        })
        .unwrap();

    // Add some usage
    rt.resource_manager
        .track(
            &ai_os_runtime::task::TaskId::new(),
            ResourceUsage {
                cpu_percent: 30.0,
                memory_bytes: 500_000,
            },
        )
        .unwrap();

    // 25% more is OK
    assert!(rt
        .resource_manager
        .check_limits(&ResourceUsage {
            cpu_percent: 20.0,
            memory_bytes: 100_000,
        })
        .is_ok());

    // 25% more would exceed 50% limit
    assert!(rt
        .resource_manager
        .check_limits(&ResourceUsage {
            cpu_percent: 25.0,
            memory_bytes: 100_000,
        })
        .is_err());
}

#[tokio::test]
async fn scheduler_priority_ordering() {
    let (app, rt) = setup().await;

    let high = rt
        .task_manager
        .create_task(None, Priority::HIGH, HashMap::new());
    let low = rt
        .task_manager
        .create_task(None, Priority::LOW, HashMap::new());
    let normal = rt
        .task_manager
        .create_task(None, Priority::NORMAL, HashMap::new());

    rt.scheduler
        .enqueue(ai_os_runtime::task::TaskHandle {
            task_id: normal.id.clone(),
            priority: Priority::NORMAL,
            session_id: None,
            created_at: normal.created_at,
        })
        .unwrap();
    rt.scheduler
        .enqueue(ai_os_runtime::task::TaskHandle {
            task_id: low.id.clone(),
            priority: Priority::LOW,
            session_id: None,
            created_at: low.created_at,
        })
        .unwrap();
    rt.scheduler
        .enqueue(ai_os_runtime::task::TaskHandle {
            task_id: high.id.clone(),
            priority: Priority::HIGH,
            session_id: None,
            created_at: high.created_at,
        })
        .unwrap();

    // Should dequeue in priority order: HIGH → NORMAL → LOW
    assert_eq!(rt.scheduler.dequeue().unwrap().task_id, high.id);
    assert_eq!(rt.scheduler.dequeue().unwrap().task_id, normal.id);
    assert_eq!(rt.scheduler.dequeue().unwrap().task_id, low.id);
}

#[tokio::test]
async fn event_bus_runtime_events() {
    let (app, rt) = setup().await;

    let fired = Arc::new(std::sync::atomic::AtomicBool::new(false));

    #[derive(Debug)]
    struct PhaseHandler {
        flag: Arc<std::sync::atomic::AtomicBool>,
    }

    #[async_trait]
    impl EventHandler<ai_os_runtime::state::RuntimePhaseChanged> for PhaseHandler {
        async fn handle(
            &self,
            _event: &ai_os_runtime::state::RuntimePhaseChanged,
        ) -> Result<(), CoreError> {
            self.flag.store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        }
    }

    typed_subscribe::<ai_os_runtime::state::RuntimePhaseChanged, PhaseHandler>(
        app.event_bus().as_ref(),
        Arc::new(PhaseHandler {
            flag: fired.clone(),
        }),
    )
    .unwrap();

    rt.start().await.unwrap();
    assert!(fired.load(std::sync::atomic::Ordering::SeqCst));
    rt.stop().await.unwrap();
}

#[tokio::test]
async fn runtime_implements_core_service() {
    let app = PlatformBuilder::new()
        .without_console_sink()
        .build()
        .await
        .unwrap();

    let rt = Runtime::new(app.event_bus().clone(), app.logger().clone());

    // Register as a Core service
    app.lifecycle().register(Arc::new(rt)).await.unwrap();

    app.run().await.unwrap();
    app.shutdown().await.unwrap();
}
