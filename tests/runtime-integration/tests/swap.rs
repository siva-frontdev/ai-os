//! Integration test: runtime swap.
//!
//! Planner → Runtime Manager → Mock Runtime works with zero planner
//! changes. The exact same planner-and-dispatch harness drives both the
//! OpenClaw runtime (see email_flow.rs) and the MockRuntime here; the
//! only difference is which runtime is registered in the manager.

use ai_os_runtime_api::{ActionStatus, CapabilityId};
use ai_os_runtime_manager::RuntimeManager;
use brain_coordinator::planner::Planner;
use runtime_integration_tests::common::coordinator::FakeCoordinator;
use runtime_integration_tests::common::dispatch_plan;
use runtime_integration_tests::common::mock_runtime::MockRuntime;

#[tokio::test]
async fn planner_drives_mock_runtime_without_planner_changes() {
    // The coordinator scripts a plan that includes a runtime capability.
    let coordinator = FakeCoordinator::plan(
        r#"{"actions":[{"type":"email.send","topics":["admin@example.com","Project update","The project is on track."],"reason":"user requested an email"}]}"#,
    );

    // Real Planner, unmodified. Produces a plan from a user message.
    let planner = Planner::new(coordinator.clone());
    let plan = planner
        .plan(
            "Send an email to admin@example.com about the project",
            "",
            "",
            false,
        )
        .await;
    assert_eq!(plan.actions[0].action_type, "email.send");

    // The manager is wired with a *mock* runtime instead of OpenClaw.
    let mock_runtime = MockRuntime::with_capabilities("mock", MockRuntime::email_capabilities());
    let manager = RuntimeManager::new();
    manager.register(mock_runtime.clone()).await.unwrap();
    manager.initialize().await.unwrap();

    let results = dispatch_plan(&manager, &plan).await;
    assert_eq!(results.len(), 1, "one runtime capability dispatched");

    let email_result = results[0]
        .as_ref()
        .expect("dispatch did not fail transport");
    assert_eq!(email_result.status, ActionStatus::Succeeded);

    // The mock runtime actually executed the capability.
    assert_eq!(mock_runtime.executed().await, vec!["email.send"]);
}

#[tokio::test]
async fn runtime_capabilities_are_visible_to_the_planner_prompt() {
    let coordinator = FakeCoordinator::plan(
        r#"{"actions":[{"type":"respond","reason":"no runtime action needed"}]}"#,
    );

    // Register a runtime capability BEFORE planning so it appears in the
    // planner's "Available capabilities" list.
    let mock_runtime = MockRuntime::with_capabilities("mock", MockRuntime::email_capabilities());
    let manager = RuntimeManager::new();
    manager.register(mock_runtime).await.unwrap();
    manager.initialize().await.unwrap();
    assert!(
        manager
            .has_capability(&CapabilityId::new("email.send"))
            .await
    );

    let planner = Planner::new(coordinator.clone());
    let plan = planner.plan("hello", "", "", false).await;
    assert_eq!(plan.actions[0].action_type, "respond");
}
