//! Integration test: capability dispatch.
//!
//! Action → Runtime Manager → OpenClaw Runtime → Mock MCP → Success.

use ai_os_runtime_api::{Action, ActionStatus, CapabilityId, RuntimeId};
use ai_os_runtime_manager::RuntimeManager;
use runtime_integration_tests::common::openclaw::{
    email_plugin_handler, email_send_tool, openclaw_runtime_with_handler,
};

fn email_action(to: &str, subject: &str, body: &str) -> Action {
    Action {
        action_id: "act-email".into(),
        capability: CapabilityId::new("email.send"),
        input: serde_json::json!({
            "to": to,
            "subject": subject,
            "body": body,
        }),
        trace_id: Some("trace-1".into()),
        deadline_ms: None,
        metadata: Default::default(),
    }
}

#[tokio::test]
async fn action_dispatch_reaches_openclaw_through_mcp() {
    let runtime = openclaw_runtime_with_handler(
        vec![email_send_tool()],
        "email.send",
        email_plugin_handler(),
    )
    .await;

    let manager = RuntimeManager::new();
    manager.register(runtime).await.unwrap();
    manager.initialize().await.unwrap();

    let result = manager
        .dispatch(email_action("admin@example.com", "Hello", "World"))
        .await
        .unwrap();

    assert!(result.is_success());
    let output = result.output.expect("success output present");
    assert_eq!(output["is_error"], false);

    // The mock email plugin returns a structured message id as text.
    let tool_text = output["content"][0].as_str().unwrap();
    let payload: serde_json::Value = serde_json::from_str(tool_text).unwrap();
    assert_eq!(payload["status"], "sent");
    assert!(payload["messageId"]
        .as_str()
        .unwrap()
        .starts_with("mock-msg-"));
}

#[tokio::test]
async fn unknown_capability_returns_typed_failure() {
    let runtime = openclaw_runtime_with_handler(
        vec![email_send_tool()],
        "email.send",
        email_plugin_handler(),
    )
    .await;

    let manager = RuntimeManager::new();
    manager.register(runtime).await.unwrap();
    manager.initialize().await.unwrap();

    let mut action = email_action("a@b.c", "s", "b");
    action.capability = CapabilityId::new("does.not_exist");
    action.action_id = "act-unknown".into();

    let result = manager.dispatch(action).await.unwrap();
    assert_eq!(result.status, ActionStatus::Failed);
    assert_eq!(result.error_code(), Some("capability_not_found"));
    assert!(!result.error.unwrap().retryable);
}

#[tokio::test]
async fn invalid_input_is_rejected_at_boundary() {
    let runtime = openclaw_runtime_with_handler(
        vec![email_send_tool()],
        "email.send",
        email_plugin_handler(),
    )
    .await;

    let manager = RuntimeManager::new();
    manager.register(runtime).await.unwrap();
    manager.initialize().await.unwrap();

    // Missing required "to"/"subject"/"body".
    let action = Action {
        action_id: "act-invalid".into(),
        capability: CapabilityId::new("email.send"),
        input: serde_json::json!({"bogus": 1}),
        trace_id: None,
        deadline_ms: None,
        metadata: Default::default(),
    };

    let result = manager.dispatch(action).await.unwrap();
    assert_eq!(result.status, ActionStatus::Failed);
    assert_eq!(result.error_code(), Some("invalid_input"));
}

#[tokio::test]
async fn runtime_id_is_reported_by_manager() {
    let runtime = openclaw_runtime_with_handler(
        vec![email_send_tool()],
        "email.send",
        email_plugin_handler(),
    )
    .await;

    let manager = RuntimeManager::new();
    manager.register(runtime).await.unwrap();
    manager.initialize().await.unwrap();

    assert_eq!(
        manager.runtime_ids().await,
        vec![RuntimeId("openclaw".into())]
    );
}
