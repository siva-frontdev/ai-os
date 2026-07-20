use execution_core::*;
use execution_sandbox::DefaultSandboxEnforcer;
use std::collections::HashMap;

fn make_plan(profile_name: &str, binding: ToolBinding) -> ExecutionPlan {
    let mut profile = SandboxProfile::new(profile_name);
    match &binding {
        ToolBinding::Subprocess { .. } => profile.isolation = IsolationLevel::Process,
        ToolBinding::Wasm { .. } => profile.isolation = IsolationLevel::Wasm,
        ToolBinding::Container { .. } => profile.isolation = IsolationLevel::Container,
    };
    ExecutionPlan {
        id: ExecutionId::new(),
        request: ExecutionRequest {
            requirement_id: "int-req".into(),
            capability_id: "int-cap".into(),
            inputs: HashMap::new(),
            context: ExecutionContext {
                session: ExecutionSession {
                    session_id: "int-s".into(),
                    user_id: "int-u".into(),
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
        },
        binding,
        sandbox_profile: profile,
        budget: ExecutionBudget::default(),
        priority: ExecutionPriority::default(),
        permissions: ExecutionPermissions::default(),
        routing_rules: Vec::new(),
        rollback_plan: None,
        state: ExecutionState::Planned,
        created_at: memory_core::Timestamp::now(),
    }
}

#[tokio::test]
async fn test_register_readonly_profile_resolves() {
    let enforcer = DefaultSandboxEnforcer::new();
    let mut profile = SandboxProfile::new("readonly-integration");
    profile.allowed_paths = vec!["/usr/bin".into(), "/bin".into()];
    profile.network_access = false;
    enforcer.register_profile(profile);

    let resolved = enforcer.resolve_profile("readonly-integration").await;
    assert!(resolved.is_ok());
    let p = resolved.unwrap();
    assert_eq!(p.name, "readonly-integration");
    assert!(!p.network_access);
}

#[tokio::test]
async fn test_subprocess_plan_allowed_path_passes() {
    let enforcer = DefaultSandboxEnforcer::new();
    let mut profile = SandboxProfile::new("sub-allowed");
    profile.allowed_paths = vec!["/usr/bin".into(), "/bin".into()];
    enforcer.register_profile(profile);

    let plan = make_plan(
        "sub-allowed",
        ToolBinding::Subprocess {
            binary: "/bin/echo".into(),
            args: vec!["hello".into()],
            env: HashMap::new(),
            working_dir: None,
            allowed_paths: vec!["/tmp".into()],
            denied_binaries: vec![],
        },
    );
    let result = enforcer.validate(&plan).await;
    assert!(result.is_ok(), "expected ok, got {:?}", result);
}

#[tokio::test]
async fn test_subprocess_plan_denied_binary_fails() {
    let enforcer = DefaultSandboxEnforcer::new();
    let mut profile = SandboxProfile::new("sub-denied");
    profile.denied_paths = vec!["/bin/rm".into()];
    enforcer.register_profile(profile);

    let plan = make_plan(
        "sub-denied",
        ToolBinding::Subprocess {
            binary: "/bin/rm".into(),
            args: vec!["-rf".into(), "/".into()],
            env: HashMap::new(),
            working_dir: None,
            allowed_paths: vec![],
            denied_binaries: vec![],
        },
    );
    let result = enforcer.validate(&plan).await;
    assert!(result.is_err(), "expected err for denied binary");
}

#[tokio::test]
async fn test_container_permissions_network_denied() {
    let enforcer = DefaultSandboxEnforcer::new();
    let mut profile = SandboxProfile::new("container-no-net");
    profile.isolation = IsolationLevel::Container;
    profile.network_access = false;
    let profile_check = profile.clone();
    enforcer.register_profile(profile);

    let binding = ToolBinding::Container {
        image: "nginx".into(),
        command: vec!["nginx".into()],
        mounts: vec![],
        network: ContainerNetwork::Host,
        pull_policy: PullPolicy::IfNotPresent,
        resource_limits: ContainerResources::default(),
    };
    let result = enforcer.check_permissions(&binding, &profile_check).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    let err_str = err.to_string();
    assert!(
        err_str.contains("network"),
        "expected network-related error, got: {}",
        err_str
    );
}

#[tokio::test]
async fn test_capabilities_match() {
    let enforcer = DefaultSandboxEnforcer::new();
    let binding = ToolBinding::Subprocess {
        binary: "/bin/ls".into(),
        args: vec![],
        env: HashMap::new(),
        working_dir: None,
        allowed_paths: vec![],
        denied_binaries: vec![],
    };
    let required = vec!["subprocess".into(), "exec".into()];
    let result = enforcer.check_capabilities(&binding, &required).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_capabilities_mismatch_fails() {
    let enforcer = DefaultSandboxEnforcer::new();
    let binding = ToolBinding::Subprocess {
        binary: "/bin/ls".into(),
        args: vec![],
        env: HashMap::new(),
        working_dir: None,
        allowed_paths: vec![],
        denied_binaries: vec![],
    };
    let required = vec!["container".into()];
    let result = enforcer.check_capabilities(&binding, &required).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_default_profile_initialized_correctly() {
    let enforcer = DefaultSandboxEnforcer::new();
    let profile = enforcer.resolve_profile("default").await;
    assert!(profile.is_ok());
    let p = profile.unwrap();
    assert_eq!(p.name, "default");
    assert_eq!(p.isolation, IsolationLevel::Process);
    assert!(p.allowed_paths.is_empty());
    assert!(!p.network_access);
}

#[tokio::test]
async fn test_validate_plan_without_registering_profile_fails() {
    let enforcer = DefaultSandboxEnforcer::new();
    let plan = make_plan(
        "never-registered",
        ToolBinding::Subprocess {
            binary: "/bin/true".into(),
            args: vec![],
            env: HashMap::new(),
            working_dir: None,
            allowed_paths: vec![],
            denied_binaries: vec![],
        },
    );
    let result = enforcer.validate(&plan).await;
    assert!(result.is_err());
}
