use criterion::{black_box, criterion_group, criterion_main, Criterion};
use execution_core::*;
use execution_sandbox::DefaultSandboxEnforcer;
use std::collections::HashMap;

fn bench_resolve_profile(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let enforcer = DefaultSandboxEnforcer::new();

    c.bench_function("resolve_profile_default", |b| {
        b.to_async(&rt).iter(|| async {
            let profile = enforcer.resolve_profile(black_box("default")).await;
            black_box(profile.unwrap());
        })
    });
}

fn bench_validate_subprocess_plan(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let enforcer = DefaultSandboxEnforcer::new();
    let mut profile = SandboxProfile::new("bench-profile");
    profile.allowed_paths = vec!["/usr/bin".into(), "/bin".into()];
    enforcer.register_profile(profile);

    let plan = ExecutionPlan {
        id: ExecutionId::new(),
        request: ExecutionRequest {
            requirement_id: "bench-req".into(),
            capability_id: "bench-cap".into(),
            inputs: HashMap::new(),
            context: ExecutionContext {
                session: ExecutionSession {
                    session_id: "bench-s".into(),
                    user_id: "bench-u".into(),
                    roles: vec!["admin".into()],
                    permissions: ExecutionPermissions::default(),
                },
                trace_id: "t".into(),
                span_id: "s".into(),
                originating_goal: None,
                originating_plan: None,
            },
            budget: ExecutionBudget::default(),
            priority: ExecutionPriority::default(),
            retry_policy: RetryPolicy::default(),
        },
        binding: ToolBinding::Subprocess {
            binary: "/bin/echo".into(),
            args: vec!["hello".into()],
            env: HashMap::new(),
            working_dir: None,
            allowed_paths: vec!["/tmp".into()],
            denied_binaries: vec![],
        },
        sandbox_profile: SandboxProfile::new("bench-profile"),
        budget: ExecutionBudget::default(),
        priority: ExecutionPriority::default(),
        permissions: ExecutionPermissions::default(),
        routing_rules: Vec::new(),
        rollback_plan: None,
        state: ExecutionState::Planned,
        created_at: memory_core::Timestamp::now(),
    };

    c.bench_function("validate_subprocess_plan", |b| {
        b.to_async(&rt).iter(|| async {
            let result = enforcer.validate(black_box(&plan)).await;
            black_box(result.unwrap());
        })
    });
}

fn bench_check_permissions_container(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let enforcer = DefaultSandboxEnforcer::new();
    let mut profile = SandboxProfile::new("bench-container");
    profile.isolation = IsolationLevel::Container;
    profile.network_access = true;
    let profile_clone = profile.clone();
    enforcer.register_profile(profile);

    let binding = ToolBinding::Container {
        image: "nginx".into(),
        command: vec!["nginx".into()],
        mounts: vec![ContainerMount {
            source: "/data".into(),
            target: "/mnt/data".into(),
            read_only: true,
        }],
        network: ContainerNetwork::Bridge,
        pull_policy: PullPolicy::IfNotPresent,
        resource_limits: ContainerResources::default(),
    };

    c.bench_function("check_permissions_container", |b| {
        b.to_async(&rt).iter(|| async {
            let result = enforcer
                .check_permissions(black_box(&binding), black_box(&profile_clone))
                .await;
            black_box(result.unwrap());
        })
    });
}

criterion_group!(
    benches,
    bench_resolve_profile,
    bench_validate_subprocess_plan,
    bench_check_permissions_container
);
criterion_main!(benches);
