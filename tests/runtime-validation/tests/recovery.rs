//! Failure detection and recovery against a real subprocess.
//!
//! A runtime's MCP server is killed mid-session; the manager must report
//! the runtime as degraded/unavailable, and dispatch must fail with a typed
//! interface error (never a silent success). A freshly-spawned replacement
//! subprocess demonstrates the restart primitive the platform uses to
//! recover.

use std::sync::Arc;

use ai_os_openclaw_runtime::mcp::transport::McpTransport;
use ai_os_runtime_api::{Action, CapabilityId, RuntimeError, RuntimeHealth};
use ai_os_runtime_manager::RuntimeManager;
use runtime_validation_tests::common::transport::spawn_mcp_server;
use runtime_validation_tests::common::{runtime_with_transport, temp_root};

fn action(capability: &str, input: serde_json::Value) -> Action {
    Action {
        action_id: uuid::Uuid::new_v4().to_string(),
        capability: CapabilityId::new(capability),
        input,
        trace_id: None,
        deadline_ms: None,
        metadata: Default::default(),
    }
}

#[tokio::test]
async fn killed_runtime_is_detected_and_dispatch_fails_typed() {
    let root = temp_root("kill");

    // Real MCP subprocess with a killable transport handle.
    let raw = spawn_mcp_server(&["email"], &root).expect("spawn real mcp server");
    let transport: Arc<dyn McpTransport> = raw.clone();
    let (runtime, _transport) = runtime_with_transport("mcp", transport);

    let manager = RuntimeManager::new();
    manager.register(Arc::new(runtime)).await.unwrap();
    manager.initialize().await.unwrap();

    // Healthy: capabilities discovered and dispatch works. `email.inject`
    // is a self-contained tool (inbound simulation) so it succeeds without
    // external credentials.
    let health = manager.health().await;
    assert_eq!(health[0].1, RuntimeHealth::Ready);
    let ok = manager
        .dispatch(action(
            "email.inject",
            serde_json::json!({
                "from": "a@b.c",
                "to": "x@y.z",
                "subject": "s",
                "body": "b",
            }),
        ))
        .await
        .unwrap();
    assert!(ok.is_success(), "dispatch before kill failed: {ok:#?}");

    // Kill the MCP server and reap it so the pipes close deterministically.
    raw.kill().await.expect("kill subprocess");
    let _ = raw.wait().await;

    // Detection: the manager must report the runtime as not Ready.
    let health = manager.health().await;
    assert!(
        !matches!(health[0].1, RuntimeHealth::Ready),
        "killed runtime should not report Ready: {:?}",
        health[0].1
    );

    // Dispatch must fail with a typed interface error, never a silent
    // success and never a panic.
    let result = manager
        .dispatch(action("email.send", serde_json::json!({"to": "a@b.c"})))
        .await;
    assert!(
        matches!(result, Err(RuntimeError::Transport(_))),
        "dispatch after kill should be a typed transport error, got {result:#?}"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn replacement_runtime_restores_service() {
    let root = temp_root("replace");

    // The restart primitive: a brand-new subprocess wrapped in a fresh
    // runtime, registered and initialized in a fresh manager, restores the
    // capability surface.
    let raw = spawn_mcp_server(&["email"], &root).expect("spawn replacement mcp server");
    let transport: Arc<dyn McpTransport> = raw.clone();
    let (runtime, _transport) = runtime_with_transport("mcp", transport);

    let manager = RuntimeManager::new();
    manager.register(Arc::new(runtime)).await.unwrap();
    manager.initialize().await.unwrap();

    let health = manager.health().await;
    assert_eq!(health[0].1, RuntimeHealth::Ready);

    let ok = manager
        .dispatch(action(
            "email.inject",
            serde_json::json!({
                "from": "b@c.d",
                "to": "x@y.z",
                "subject": "s",
                "body": "b",
            }),
        ))
        .await
        .unwrap();
    assert!(
        ok.is_success(),
        "replacement runtime dispatch failed: {ok:#?}"
    );

    let _ = std::fs::remove_dir_all(&root);
}
