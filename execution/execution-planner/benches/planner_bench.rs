use criterion::{black_box, criterion_group, criterion_main, Criterion};
use execution_core::*;
use execution_planner::DefaultExecutionPlanner;
use execution_registry::{DefaultToolResolver, InMemoryToolRegistry};
use execution_sandbox::DefaultSandboxEnforcer;
use std::collections::HashMap;
use std::sync::Arc;

fn make_request(capability_id: &str) -> ExecutionRequest {
    ExecutionRequest {
        requirement_id: format!("req-{}", capability_id),
        capability_id: capability_id.into(),
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
        budget: ExecutionBudget::default(),
        priority: ExecutionPriority::default(),
        retry_policy: RetryPolicy::default(),
    }
}

fn setup_planner() -> DefaultExecutionPlanner {
    let registry = Arc::new(InMemoryToolRegistry::new());
    let binding = ToolBinding::Subprocess {
        binary: "/usr/bin/echo".into(),
        args: Vec::new(),
        env: HashMap::new(),
        working_dir: None,
        allowed_paths: Vec::new(),
        denied_binaries: Vec::new(),
    };
    let caps = vec![ExecutionCapability {
        id: "text.echo".into(),
        name: "Echo".into(),
        description: String::new(),
        input_schema: HashMap::new(),
        output_schema: HashMap::new(),
        required_permissions: Vec::new(),
    }];

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(registry.register(binding, caps)).unwrap();

    let resolver = Arc::new(DefaultToolResolver::new(registry.clone()));
    let sandbox = Arc::new(DefaultSandboxEnforcer::new());

    DefaultExecutionPlanner::new(registry, resolver, sandbox)
}

fn bench_plan_creation(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let planner = setup_planner();
    let request = make_request("text.echo");

    c.bench_function("plan_creation", |b| {
        b.to_async(&rt)
            .iter(|| planner.plan(black_box(request.clone())));
    });
}

fn bench_batch_planning(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let planner = setup_planner();
    let requests: Vec<ExecutionRequest> = (0..10)
        .map(|i| {
            let mut req = make_request("text.echo");
            req.requirement_id = format!("req-{}", i);
            req
        })
        .collect();

    c.bench_function("batch_plan_10", |b| {
        b.to_async(&rt)
            .iter(|| planner.plan_batch(black_box(requests.clone())));
    });
}

fn bench_dependency_graph(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let planner = setup_planner();
    let requests: Vec<ExecutionRequest> = (0..10)
        .map(|i| {
            let mut req = make_request("text.echo");
            req.requirement_id = format!("req-{}", i);
            req
        })
        .collect();
    let plans = rt.block_on(planner.plan_batch(requests)).unwrap();

    c.bench_function("dependency_graph_10", |b| {
        b.to_async(&rt)
            .iter(|| planner.build_dependency_graph(black_box(&plans)));
    });
}

criterion_group!(
    name = benches;
    config = Criterion::default().sample_size(10);
    targets = bench_plan_creation, bench_batch_planning, bench_dependency_graph
);
criterion_main!(benches);
