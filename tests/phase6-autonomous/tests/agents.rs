//! Phase 6 — Autonomous Agent: multi-agent coordination and delegation.
//!
//! Proves the agent registry can select the right agent for a task by
//! role or capability, and that delegation round-trips through the real
//! `AgentRegistry` with scripted providers.

use std::sync::Arc;

use brain_coordinator::agent_registry::AgentRegistry;
use brain_core::delegation::{AgentRole, DelegationStatus};
use brain_core::ids::AgentId;
use phase6_autonomous::stubs::ScriptedAgent;

fn registry_with_agents() -> AgentRegistry {
    let registry = AgentRegistry::new();
    // Execution agent with email + filesystem capabilities.
    registry.register(Arc::new(ScriptedAgent::new(
        "executor",
        AgentRole::Execution,
        vec!["email.send", "filesystem.write"],
        true,
    )));
    // Researcher agent with search capability.
    registry.register(Arc::new(ScriptedAgent::new(
        "researcher",
        AgentRole::Researcher,
        vec!["search_web"],
        true,
    )));
    registry
}

#[test]
fn finds_agent_by_capability_for_tool_selection() {
    let registry = registry_with_agents();

    let (agent_id, role) = registry
        .find_by_capability("email.send")
        .expect("agent found for email.send");
    assert_eq!(role, AgentRole::Execution);
    assert!(registry.has_agent(&agent_id));
}

#[test]
fn finds_agent_by_role() {
    let registry = registry_with_agents();

    let researcher = registry
        .find_by_role(AgentRole::Researcher)
        .expect("researcher found");
    assert!(registry.has_agent(&researcher));
}

#[test]
fn unknown_capability_has_no_agent() {
    let registry = registry_with_agents();
    assert!(
        registry
            .find_by_capability("nonexistent.capability")
            .is_none(),
        "no agent should be selected for an unknown capability"
    );
}

#[test]
fn lists_all_registered_agents() {
    let registry = registry_with_agents();
    let agents = registry.list_agents();
    assert_eq!(agents.len(), 2, "two agents registered");
    let names: Vec<&str> = agents.iter().map(|a| a.name.as_str()).collect();
    assert!(names.contains(&"executor"));
    assert!(names.contains(&"researcher"));
}

#[tokio::test]
async fn successful_delegation_returns_result() {
    let registry = registry_with_agents();
    let (agent_id, _) = registry
        .find_by_capability("email.send")
        .expect("agent found");

    let goal_id = brain_core::ids::GoalId::new();
    let request = brain_core::delegation::DelegationRequest {
        delegation_id: brain_core::ids::GoalId::new(),
        goal_id,
        agent_id,
        task_description: "send weekly digest".into(),
        expected_outcome: "email sent".into(),
        timeout_ms: 60_000,
        priority: 5,
    };

    let result = registry
        .delegate(&agent_id, request)
        .await
        .expect("delegation succeeds");
    assert!(result.success);
    assert_eq!(result.agent_id, agent_id);

    let status = registry
        .delegation_status(&result.delegation_id)
        .await
        .expect("status query");
    assert_eq!(
        status,
        DelegationStatus::Completed,
        "success removes delegation"
    );
}

#[tokio::test]
async fn failed_delegation_tracks_active_delegation() {
    let registry = AgentRegistry::new();
    let agent_id = registry.register(Arc::new(ScriptedAgent::new(
        "flaky",
        AgentRole::Execution,
        vec!["filesystem.write"],
        false,
    )));

    let request = brain_core::delegation::DelegationRequest {
        delegation_id: brain_core::ids::GoalId::new(),
        goal_id: brain_core::ids::GoalId::new(),
        agent_id,
        task_description: "risky write".into(),
        expected_outcome: "written".into(),
        timeout_ms: 60_000,
        priority: 5,
    };

    let result = registry
        .delegate(&agent_id, request)
        .await
        .expect("delegation attempted");
    assert!(!result.success);
    assert!(result.error.is_some(), "failure carries an error");

    let status = registry
        .delegation_status(&result.delegation_id)
        .await
        .expect("status query");
    assert_eq!(
        status,
        DelegationStatus::InProgress,
        "failure kept as active"
    );
}

#[tokio::test]
async fn unregister_removes_agent() {
    let registry = AgentRegistry::new();
    let agent_id: AgentId = registry.register(Arc::new(ScriptedAgent::new(
        "temp",
        AgentRole::Coder,
        vec!["github.create_issue"],
        true,
    )));

    assert!(registry.has_agent(&agent_id));
    assert!(registry.unregister(&agent_id), "unregister succeeds");
    assert!(!registry.has_agent(&agent_id));
    assert!(!registry.unregister(&agent_id), "second unregister no-op");
}

#[tokio::test]
async fn orchestrator_delegates_to_agent() {
    use brain_coordinator::BrainOrchestrator;
    use phase6_autonomous::stubs::{new_goal_store, AllowAllPolicyEvaluator, EmptyToolRegistry};

    let brain = BrainOrchestrator::new(
        Box::new(AllowAllPolicyEvaluator),
        Arc::new(EmptyToolRegistry),
        new_goal_store(),
        Arc::new(phase6_autonomous::stubs::ScriptedReasoningService::new(
            phase6_autonomous::stubs::scripted_reasoning(
                "research",
                "research task for agent",
                vec!["search_web"],
            ),
        )),
    );

    let agent_id = brain.agent_registry().register(Arc::new(ScriptedAgent::new(
        "researcher",
        AgentRole::Researcher,
        vec!["search_web"],
        true,
    )));

    let goal_id = brain_core::ids::GoalId::new();
    let result = brain
        .delegate_to_agent(agent_id, goal_id, "find sources", "sources found", 60_000)
        .await
        .expect("delegation through orchestrator");
    assert!(result.success);
    assert_eq!(result.agent_id, agent_id);
}
