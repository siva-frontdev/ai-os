use execution_core::*;
use execution_runner::*;
use std::collections::HashMap;
use std::sync::Arc;

fn make_plan(binary: &str, args: &[&str]) -> ExecutionPlan {
    ExecutionPlan {
        id: ExecutionId::new(),
        request: ExecutionRequest {
            requirement_id: "test".into(),
            capability_id: "test".into(),
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

#[tokio::test]
async fn test_subprocess_echo() {
    let runner = SubprocessRunner::new();
    let plan = make_plan("/bin/echo", &["hello world"]);
    let output = runner.execute(&plan).await.unwrap();
    assert_eq!(output.exit_code, 0);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "hello world"
    );
}

#[tokio::test]
async fn test_subprocess_true() {
    let runner = SubprocessRunner::new();
    let plan = make_plan("/bin/true", &[]);
    let output = runner.execute(&plan).await.unwrap();
    assert_eq!(output.exit_code, 0);
}

#[tokio::test]
async fn test_subprocess_false() {
    let runner = SubprocessRunner::new();
    let plan = make_plan("/bin/false", &[]);
    let result = runner.execute(&plan).await;
    assert!(result.is_err());
    match result {
        Err(ExecutionError::InvalidExitCode { exit_code }) => assert_eq!(exit_code, 1),
        _ => panic!("expected InvalidExitCode, got: {:?}", result),
    }
}

#[tokio::test]
async fn test_subprocess_cancellation() {
    let runner = Arc::new(SubprocessRunner::new());
    let plan = make_plan("/bin/sleep", &["10"]);
    let execution_id = plan.id;

    let runner_for_exec = runner.clone();
    let handle = tokio::spawn(async move { runner_for_exec.execute(&plan).await });

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    let cancel_result = runner.cancel(&execution_id).await;
    assert!(
        cancel_result.is_ok(),
        "cancel should succeed: {:?}",
        cancel_result.err()
    );

    let exec_result = handle.await.unwrap();
    assert!(
        matches!(exec_result, Err(ExecutionError::SignalTerminated { .. })),
        "expected SignalTerminated, got: {:?}",
        exec_result
    );
}

#[tokio::test]
async fn test_wasm_not_implemented() {
    let runner = DefaultWasmRunner;
    let plan = make_plan("/bin/true", &[]);
    let result = runner.execute(&plan).await;
    match result {
        Err(ExecutionError::BackendUnavailable(msg)) => {
            assert!(!msg.is_empty());
        }
        _ => panic!("expected BackendUnavailable"),
    }

    let cancel_result = runner.cancel(&ExecutionId::new()).await;
    match cancel_result {
        Err(ExecutionError::BackendUnavailable(_)) => {}
        _ => panic!("expected BackendUnavailable"),
    }

    let health_result = runner.health().await;
    match health_result {
        Err(ExecutionError::BackendUnavailable(_)) => {}
        _ => panic!("expected BackendUnavailable"),
    }

    assert_eq!(runner.backend_type(), "wasm");
}

#[tokio::test]
async fn test_container_not_implemented() {
    let runner = DefaultContainerRunner;
    let plan = make_plan("/bin/true", &[]);
    let result = runner.execute(&plan).await;
    match result {
        Err(ExecutionError::BackendUnavailable(msg)) => {
            assert!(!msg.is_empty());
        }
        _ => panic!("expected BackendUnavailable"),
    }

    let cancel_result = runner.cancel(&ExecutionId::new()).await;
    match cancel_result {
        Err(ExecutionError::BackendUnavailable(_)) => {}
        _ => panic!("expected BackendUnavailable"),
    }

    let health_result = runner.health().await;
    match health_result {
        Err(ExecutionError::BackendUnavailable(_)) => {}
        _ => panic!("expected BackendUnavailable"),
    }

    assert_eq!(runner.backend_type(), "container");
}

#[tokio::test]
async fn test_runner_factory_routes_subprocess() {
    let factory = DefaultRunnerFactory;
    let binding = ToolBinding::Subprocess {
        binary: "/bin/true".into(),
        args: Vec::new(),
        env: HashMap::new(),
        working_dir: None,
        allowed_paths: Vec::new(),
        denied_binaries: Vec::new(),
    };
    let runner = factory.create_runner(&binding).await.unwrap();
    assert_eq!(runner.backend_type(), "subprocess");
}

#[tokio::test]
async fn test_runner_factory_routes_wasm() {
    let factory = DefaultRunnerFactory;
    let binding = ToolBinding::Wasm {
        module_path: "test.wasm".into(),
        function_name: "main".into(),
        input_format: WasmInputFormat::Json,
        fuel_limit: None,
        precompile: false,
    };
    let runner = factory.create_runner(&binding).await.unwrap();
    assert_eq!(runner.backend_type(), "wasm");
}

#[tokio::test]
async fn test_runner_factory_routes_container() {
    let factory = DefaultRunnerFactory;
    let binding = ToolBinding::Container {
        image: "alpine:latest".into(),
        command: vec!["true".into()],
        mounts: Vec::new(),
        network: ContainerNetwork::None,
        pull_policy: PullPolicy::IfNotPresent,
        resource_limits: ContainerResources::default(),
    };
    let runner = factory.create_runner(&binding).await.unwrap();
    assert_eq!(runner.backend_type(), "container");
}
