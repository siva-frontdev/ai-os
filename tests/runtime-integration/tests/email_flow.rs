//! Integration test: the email flow end to end.
//!
//! Telegram → AI-OS (World Understanding → Planner → Decision)
//!        → Capability Registry → Runtime Manager → OpenClaw Runtime
//!        → MCP → Mock Email Plugin → Result
//!        → AI-OS → Telegram Reply
//!
//! The `MockMcpServer` plays the role of the OpenClaw email plugin; the
//! same wire contract (MCP tools/list + tools/call) is what a real
//! OpenClaw email extension would speak.

use ai_os_runtime_api::{ActionStatus, CapabilityId};
use ai_os_runtime_manager::RuntimeManager;
use brain_coordinator::planner::Planner;
use runtime_integration_tests::common::coordinator::FakeCoordinator;
use runtime_integration_tests::common::dispatch_plan;
use runtime_integration_tests::common::openclaw::{
    email_plugin_handler, email_send_tool, openclaw_runtime_with_handler,
};

#[tokio::test]
async fn telegram_to_email_send_and_back() {
    // 1. A message arrives from a user (simulating the Telegram channel
    //    producing an inbound observation).
    let telegram_message = "send an email to admin@example.com about the project";

    // 2. AI-OS cognitive loop plans. The (fake) intelligence coordinator
    //    returns an email.send plan; the real Planner parses it.
    let coordinator = FakeCoordinator::plan(
        r#"{"actions":[{"type":"email.send","topics":["admin@example.com","Project update","The project is on track."],"reason":"user requested an email"}]}"#,
    )
    .with_reply("Email sent successfully!");

    let planner = Planner::new(coordinator.clone());
    let plan = planner.plan(telegram_message, "", "", false).await;
    assert_eq!(plan.actions[0].action_type, "email.send");

    // 3. Runtime Manager owns the OpenClaw Runtime, which talks MCP to a
    //    mock email plugin (in place of a real OpenClaw email extension).
    let runtime = openclaw_runtime_with_handler(
        vec![email_send_tool()],
        "email.send",
        email_plugin_handler(),
    )
    .await;

    let manager = RuntimeManager::new();
    manager.register(runtime).await.unwrap();
    manager.initialize().await.unwrap();

    // 4. Dispatch the plan through the manager.
    let results = dispatch_plan(&manager, &plan).await;
    assert_eq!(results.len(), 1);

    let email_result = results[0]
        .as_ref()
        .expect("dispatch did not fail transport");
    assert_eq!(email_result.status, ActionStatus::Succeeded);

    // The result carries the plugin's structured payload.
    let output = email_result.output.as_ref().expect("output present");
    let tool_text = output["content"][0].as_str().unwrap();
    let payload: serde_json::Value = serde_json::from_str(tool_text).unwrap();
    assert_eq!(payload["status"], "sent");

    // 5. The email plugin pushed an inbound observation back through MCP
    //    (delivery confirmation) — AI-OS's `observe()` path.
    let observations = manager.observe().await;
    assert!(
        observations
            .iter()
            .any(|o| o.source == CapabilityId::new("email.receive")),
        "expected a delivery observation from email.receive, got {observations:#?}"
    );

    // 6. AI-OS forms the Telegram reply.
    let reply = coordinator.simulate_reply().await;
    assert_eq!(reply, "Email sent successfully!");
}

#[tokio::test]
async fn capability_discovery_makes_email_send_plannable() {
    // Proves the manager exposes the email capability that the planner
    // can then address — the merge step of the capability flow.
    let runtime = openclaw_runtime_with_handler(
        vec![email_send_tool()],
        "email.send",
        email_plugin_handler(),
    )
    .await;

    let manager = RuntimeManager::new();
    manager.register(runtime).await.unwrap();
    manager.initialize().await.unwrap();

    let mut seen = false;
    let merged = manager
        .merge_capabilities(|capability| {
            if capability.id == CapabilityId::new("email.send") {
                seen = true;
            }
        })
        .await;
    assert_eq!(merged, 1);
    assert!(seen);
}
