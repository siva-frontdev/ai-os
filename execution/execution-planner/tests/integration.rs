use execution_core::*;
use execution_planner::{BatchPlanner, DefaultExecutionPlanner, PlannerError};
use execution_registry::{DefaultToolResolver, InMemoryToolRegistry};
use execution_sandbox::DefaultSandboxEnforcer;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

fn make_subprocess_binding(binary: &str) -> ToolBinding {
    ToolBinding::Subprocess {
        binary: binary.into(),
        args: Vec::new(),
        env: HashMap::new(),
        working_dir: None,
        allowed_paths: Vec::new(),
        denied_binaries: Vec::new(),
    }
}

fn make_capability(id: &str, name: &str) -> ExecutionCapability {
    ExecutionCapability {
        id: id.into(),
        name: name.into(),
        description: String::new(),
        input_schema: HashMap::new(),
        output_schema: HashMap::new(),
        required_permissions: Vec::new(),
    }
}

fn make_request(capability_id: &str) -> ExecutionRequest {
    ExecutionRequest {
        requirement_id: format!("req-{}", capability_id),
        capability_id: capability_id.into(),
        inputs: HashMap::new(),
        context: ExecutionContext {
            session: ExecutionSession {
                session_id: "test-session".into(),
                user_id: "test-user".into(),
                roles: vec!["admin".into()],
                permissions: ExecutionPermissions::default(),
            },
            trace_id: "trace-1".into(),
            span_id: "span-1".into(),
            originating_goal: None,
            originating_plan: None,
        },
        budget: ExecutionBudget::default(),
        priority: ExecutionPriority::default(),
        retry_policy: RetryPolicy::default(),
    }
}

async fn setup_planner() -> DefaultExecutionPlanner {
    let registry = Arc::new(InMemoryToolRegistry::new());
    let binding = make_subprocess_binding("/usr/bin/echo");
    let caps = vec![make_capability("text.echo", "Echo Text")];
    registry.register(binding, caps).await.unwrap();

    let resolver: Arc<dyn ToolResolver> = Arc::new(DefaultToolResolver::new(registry.clone()));
    let sandbox: Arc<dyn SandboxEnforcer> = Arc::new(DefaultSandboxEnforcer::new());

    DefaultExecutionPlanner::new(registry, resolver, sandbox)
}

#[tokio::test]
async fn test_plan_subprocess_request() {
    let planner = setup_planner().await;
    let request = make_request("text.echo");
    let plan = planner.plan(request).await.unwrap();

    assert_eq!(plan.state, ExecutionState::Planned);
    match &plan.binding {
        ToolBinding::Subprocess { binary, .. } => {
            assert_eq!(binary, "/usr/bin/echo");
        }
        _ => panic!("expected Subprocess binding"),
    }
    assert_eq!(plan.sandbox_profile.name, "default");
    assert_eq!(plan.routing_rules.len(), 1);
    assert_eq!(plan.routing_rules[0].destinations.len(), 2);
}

#[tokio::test]
async fn test_plan_nonexistent_capability_returns_error() {
    let planner = setup_planner().await;
    let request = make_request("nonexistent.capability");
    let result = planner.plan(request).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_plan_batch_of_five() {
    let planner = setup_planner().await;
    let requests: Vec<ExecutionRequest> = (0..5)
        .map(|i| {
            let mut req = make_request("text.echo");
            req.requirement_id = format!("req-{}", i);
            req
        })
        .collect();

    let plans = planner.plan_batch(requests).await.unwrap();
    assert_eq!(plans.len(), 5);
    let ids: HashSet<ExecutionId> = plans.iter().map(|p| p.id).collect();
    assert_eq!(ids.len(), 5);

    for plan in &plans {
        assert_eq!(plan.state, ExecutionState::Planned);
    }
}

#[tokio::test]
async fn test_build_dependency_graph_no_dependencies() {
    let planner = setup_planner().await;
    let request = make_request("text.echo");
    let plan = planner.plan(request).await.unwrap();

    let graph = planner.build_dependency_graph(&[plan]).await.unwrap();
    assert_eq!(graph.nodes.len(), 1);
    assert!(graph.edges.is_empty());
}

#[tokio::test]
async fn test_batch_planner_parallel_groups() {
    let planner = setup_planner().await;
    let batch_planner = BatchPlanner::new(planner.clone());

    let p1 = planner.plan(make_request("text.echo")).await.unwrap();
    let p2 = planner.plan(make_request("text.echo")).await.unwrap();

    let groups = batch_planner
        .identify_parallel_groups(&[p1, p2])
        .await
        .unwrap();
    assert!(!groups.is_empty());
    assert_eq!(groups[0].len(), 2);
}

#[tokio::test]
async fn test_batch_planner_exceeds_max_size() {
    let planner = setup_planner().await.with_max_batch_size(2);
    let batch_planner = BatchPlanner::new(planner);
    let requests: Vec<ExecutionRequest> = (0..5).map(|_| make_request("text.echo")).collect();

    let result = batch_planner.plan_batch(requests).await;
    assert!(result.is_err());
    match result {
        Err(PlannerError::BatchTooLarge { count, max }) => {
            assert_eq!(count, 5);
            assert_eq!(max, 2);
        }
        _ => panic!("expected BatchTooLarge"),
    }
}

#[tokio::test]
async fn test_expand_requirements_with_inputs() {
    let planner = setup_planner().await;
    let mut request = make_request("transform+validate");
    request.requirement_id = "req".into();
    request.inputs.insert("source".into(), "data".into());

    let expanded = planner.expand_requirements(request).await.unwrap();
    assert_eq!(expanded.len(), 2);
    assert_eq!(expanded[0].capability_id, "transform");
    assert_eq!(expanded[1].capability_id, "validate");
    assert_eq!(expanded[0].inputs.get("source").unwrap(), "data");
    assert_eq!(expanded[1].inputs.get("source").unwrap(), "data");
}
