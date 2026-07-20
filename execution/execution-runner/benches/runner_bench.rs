use criterion::{black_box, criterion_group, criterion_main, Criterion};
use execution_core::*;
use execution_runner::SubprocessRunner;
use std::collections::HashMap;

fn make_plan(binary: &str, args: &[&str]) -> ExecutionPlan {
    ExecutionPlan {
        id: ExecutionId::new(),
        request: ExecutionRequest {
            requirement_id: "bench".into(),
            capability_id: "bench".into(),
            inputs: HashMap::new(),
            context: ExecutionContext {
                session: ExecutionSession {
                    session_id: "bench-session".into(),
                    user_id: "bench-user".into(),
                    roles: vec!["admin".into()],
                    permissions: ExecutionPermissions::default(),
                },
                trace_id: "trace-bench".into(),
                span_id: "span-bench".into(),
                originating_goal: None,
                originating_plan: None,
            },
            budget: ExecutionBudget::new(10_000),
            priority: ExecutionPriority::NORMAL,
            retry_policy: RetryPolicy::none(),
        },
        binding: ToolBinding::Subprocess {
            binary: binary.into(),
            args: args.iter().map(|s| s.to_string()).collect(),
            env: HashMap::new(),
            working_dir: None,
            allowed_paths: Vec::new(),
            denied_binaries: Vec::new(),
        },
        sandbox_profile: SandboxProfile::new("default"),
        budget: ExecutionBudget::new(10_000),
        priority: ExecutionPriority::NORMAL,
        permissions: ExecutionPermissions::default(),
        routing_rules: Vec::new(),
        rollback_plan: None,
        state: ExecutionState::Planned,
        created_at: memory_core::Timestamp::now(),
    }
}

fn bench_subprocess_spawn(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");

    c.bench_function("subprocess_spawn_true", |b| {
        b.to_async(&rt).iter(|| async {
            let runner = SubprocessRunner::new();
            let plan = make_plan("/bin/true", &[]);
            let result = runner.execute(&plan).await;
            black_box(result)
        });
    });
}

criterion_group!(benches, bench_subprocess_spawn);
criterion_main!(benches);
