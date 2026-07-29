use criterion::{black_box, criterion_group, criterion_main, Criterion};
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
use tokio::runtime::Runtime;
use tokio::sync::mpsc;

fn make_plan(priority: ExecutionPriority) -> ExecutionPlan {
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

fn bench_dispatch_single(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let (plan_tx, _plan_rx) = mpsc::channel(1024);
    let runner = Arc::new(DefaultRunnerFactory);
    let sandbox = Arc::new(DefaultSandboxEnforcer::new());
    let policy = DispatchPolicy {
        max_concurrency: 100,
        per_backend_concurrency: HashMap::new(),
        enable_fallback: true,
        queue_capacity: 10000,
    };
    let d = Arc::new(DefaultDispatcher::new(runner, sandbox, plan_tx, policy));

    let plan = make_plan(ExecutionPriority::NORMAL);

    c.bench_function("dispatch_single", |b| {
        b.to_async(&rt).iter(|| {
            let d = d.clone();
            let plan = make_plan(ExecutionPriority::NORMAL);
            async move {
                let _handle = black_box(d.dispatch(plan).await.unwrap());
            }
        })
    });
}

fn bench_dispatch_batch(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let (plan_tx, _plan_rx) = mpsc::channel(1024);
    let runner = Arc::new(DefaultRunnerFactory);
    let sandbox = Arc::new(DefaultSandboxEnforcer::new());
    let policy = DispatchPolicy {
        max_concurrency: 50,
        per_backend_concurrency: HashMap::new(),
        enable_fallback: true,
        queue_capacity: 10000,
    };
    let d = Arc::new(DefaultDispatcher::new(runner, sandbox, plan_tx, policy));

    let plans: Vec<ExecutionPlan> = (0..10)
        .map(|i| {
            let p = if i < 3 {
                ExecutionPriority::CRITICAL
            } else {
                ExecutionPriority::NORMAL
            };
            make_plan(p)
        })
        .collect();

    c.bench_function("dispatch_batch_10", |b| {
        b.to_async(&rt).iter(|| {
            let d = d.clone();
            let plans = plans.clone();
            async move {
                for plan in plans {
                    let _handle = black_box(d.dispatch(plan).await.unwrap());
                }
            }
        })
    });
}

fn bench_queue_and_cancel(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let (plan_tx, _plan_rx) = mpsc::channel(1024);
    let runner = Arc::new(DefaultRunnerFactory);
    let sandbox = Arc::new(DefaultSandboxEnforcer::new());
    let policy = DispatchPolicy {
        max_concurrency: 0,
        per_backend_concurrency: HashMap::new(),
        enable_fallback: true,
        queue_capacity: 10000,
    };
    let d = Arc::new(DefaultDispatcher::new(runner, sandbox, plan_tx, policy));

    c.bench_function("queue_and_cancel", |b| {
        b.to_async(&rt).iter(|| {
            let d = d.clone();
            async move {
                let plan = make_plan(ExecutionPriority::NORMAL);
                let id = plan.id;
                let _handle = d.dispatch(plan).await.unwrap();
                let _ = d.cancel(id).await;
            }
        })
    });
}

fn bench_queue_depth(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let (plan_tx, _plan_rx) = mpsc::channel(1024);
    let runner = Arc::new(DefaultRunnerFactory);
    let sandbox = Arc::new(DefaultSandboxEnforcer::new());
    let policy = DispatchPolicy {
        max_concurrency: 0,
        per_backend_concurrency: HashMap::new(),
        enable_fallback: true,
        queue_capacity: 10000,
    };
    let d = Arc::new(DefaultDispatcher::new(runner, sandbox, plan_tx, policy));

    // Pre-populate queue
    let rt_pop = Runtime::new().unwrap();
    rt_pop.block_on(async {
        for _ in 0..100 {
            let _ = d.dispatch(make_plan(ExecutionPriority::NORMAL)).await;
        }
    });

    c.bench_function("queue_depth_100", |b| {
        b.to_async(&rt).iter(|| {
            let d = d.clone();
            async move {
                let depth = black_box(d.queue_depth().await);
                assert_eq!(depth, 100);
            }
        })
    });
}

criterion_group!(
    benches,
    bench_dispatch_single,
    bench_dispatch_batch,
    bench_queue_and_cancel,
    bench_queue_depth,
);
criterion_main!(benches);
