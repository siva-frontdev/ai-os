//! Real-subprocess bootstrap, capability discovery, dispatch, and
//! observation ingestion.
//!
//! These tests drive the actual `ai-os-mcp-server` binary as a spawned
//! subprocess through the full Phase 3 wiring: `RuntimeBootstrap` →
//! `RuntimeManager` → `OpenClawRuntime` → MCP stdio → real plugins.

use std::collections::HashMap;

use ai_os_runtime_api::{Action, CapabilityId, ObservationKind, RuntimeHealth};
use ai_os_runtime_bootstrap::config::{McpRuntimeConfig, RuntimeInstanceConfig};
use ai_os_runtime_bootstrap::{RuntimeBootstrap, RuntimeBootstrapConfig};
use runtime_validation_tests::common::{
    mcp_runtime_config, mcp_server_bin, temp_root, wait_for_observations,
};

fn bootstrap_with_all_plugins(root: &std::path::Path) -> RuntimeBootstrap {
    let config = RuntimeBootstrapConfig {
        instances: vec![RuntimeInstanceConfig::Mcp(McpRuntimeConfig {
            id: "mcp".into(),
            command: mcp_server_bin().display().to_string(),
            args: vec![
                "--plugins".into(),
                "email,filesystem,github,calendar,telegram,whatsapp".into(),
                "--root".into(),
                root.display().to_string(),
            ],
            allow_tools: None,
            capability_overrides: HashMap::new(),
        })],
    };
    RuntimeBootstrap::new(config)
}

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
async fn bootstrap_discovers_real_plugin_capabilities() {
    let root = temp_root("bootstrap");
    let bootstrap = bootstrap_with_all_plugins(&root);
    let summary = bootstrap.initialize().await.unwrap();

    assert_eq!(summary.runtimes.len(), 1);
    assert_eq!(summary.runtimes[0].to_string(), "mcp");
    assert!(matches!(summary.health[0].1, RuntimeHealth::Ready));

    let names: Vec<&str> = summary.capabilities.iter().map(|c| c.id.as_str()).collect();
    for expected in [
        "email.send",
        "email.inject",
        "filesystem.read",
        "filesystem.write",
        "filesystem.search",
        "github.create_issue",
        "github.read_repository",
        "calendar.read",
        "calendar.create_event",
        "telegram.send",
        "telegram.inject_inbound",
        "whatsapp.send",
        "whatsapp.receive",
    ] {
        assert!(
            names.contains(&expected),
            "missing capability {expected}: {names:?}"
        );
    }

    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn bootstrap_capabilities_merge_into_external_registry() {
    let root = temp_root("merge");
    let bootstrap = bootstrap_with_all_plugins(&root);
    bootstrap.initialize().await.unwrap();

    let mut collected = Vec::new();
    let count = bootstrap
        .manager()
        .merge_capabilities(|c| collected.push(c.id.clone()))
        .await;
    assert_eq!(count, 13);
    assert!(collected.contains(&CapabilityId::new("email.send")));
    assert!(collected.contains(&CapabilityId::new("telegram.inject_inbound")));
    assert!(collected.contains(&CapabilityId::new("whatsapp.send")));

    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn dispatch_email_send_against_real_plugin() {
    let root = temp_root("email");
    let bootstrap = bootstrap_with_all_plugins(&root);
    bootstrap.initialize().await.unwrap();

    let result = bootstrap
        .manager()
        .dispatch(action(
            "email.send",
            serde_json::json!({
                "to": "admin@example.com",
                "subject": "Validation",
                "body": "This came through the real MCP subprocess.",
            }),
        ))
        .await
        .unwrap();
    // The Gmail provider is unconfigured in the test environment. It must
    // fail honestly with a structured error, never claim delivery.
    assert!(
        !result.is_success(),
        "email.send must not succeed without Gmail credentials: {result:#?}"
    );
    assert_eq!(result.status, ai_os_runtime_api::ActionStatus::Failed);
    assert_eq!(result.error_code(), Some("tool_error"));
    assert!(
        result
            .error
            .unwrap()
            .message
            .contains("ConfigurationMissing"),
        "expected a structured ConfigurationMissing error"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn dispatch_whatsapp_send_against_real_plugin() {
    let root = temp_root("whatsapp");
    let bootstrap = bootstrap_with_all_plugins(&root);
    bootstrap.initialize().await.unwrap();

    let result = bootstrap
        .manager()
        .dispatch(action(
            "whatsapp.send",
            serde_json::json!({
                "to": "+15551234567",
                "text": "hello from validation",
            }),
        ))
        .await
        .unwrap();
    // The WhatsApp provider is unconfigured in the test environment. It must
    // fail honestly with a structured error, never claim delivery.
    assert!(
        !result.is_success(),
        "whatsapp.send must not succeed without credentials: {result:#?}"
    );
    assert_eq!(result.status, ai_os_runtime_api::ActionStatus::Failed);
    assert_eq!(result.error_code(), Some("tool_error"));
    assert!(
        result
            .error
            .unwrap()
            .message
            .contains("ConfigurationMissing"),
        "expected a structured ConfigurationMissing error"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn dispatch_filesystem_write_then_read_against_real_plugin() {
    let root = temp_root("fs");
    let bootstrap = bootstrap_with_all_plugins(&root);
    bootstrap.initialize().await.unwrap();

    let write = bootstrap
        .manager()
        .dispatch(action(
            "filesystem.write",
            serde_json::json!({"path": "notes/a.txt", "content": "hello from validation"}),
        ))
        .await
        .unwrap();
    assert!(write.is_success(), "filesystem.write failed: {write:#?}");

    let read = bootstrap
        .manager()
        .dispatch(action(
            "filesystem.read",
            serde_json::json!({"path": "notes/a.txt"}),
        ))
        .await
        .unwrap();
    assert!(read.is_success(), "filesystem.read failed: {read:#?}");
    let output = read.output.unwrap();
    let text = output["content"][0].as_str().unwrap();
    let payload: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(payload["content"], "hello from validation");
    assert_eq!(payload["bytes"], 21);

    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn dispatch_calendar_and_github_against_real_plugin() {
    let root = temp_root("calgh");
    let bootstrap = bootstrap_with_all_plugins(&root);
    bootstrap.initialize().await.unwrap();

    let calendar = bootstrap
        .manager()
        .dispatch(action(
            "calendar.read",
            serde_json::json!({"date": "2026-08-06"}),
        ))
        .await
        .unwrap();
    assert!(calendar.is_success(), "calendar.read failed: {calendar:#?}");

    let github = bootstrap
        .manager()
        .dispatch(action(
            "github.create_issue",
            serde_json::json!({"title": "found during validation", "body": "details"}),
        ))
        .await
        .unwrap();
    assert!(
        github.is_success(),
        "github.create_issue failed: {github:#?}"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn inbound_telegram_message_becomes_an_observation() {
    let root = temp_root("inbound");
    let bootstrap = bootstrap_with_all_plugins(&root);
    bootstrap.initialize().await.unwrap();

    let dispatch = bootstrap
        .manager()
        .dispatch(action(
            "telegram.inject_inbound",
            serde_json::json!({
                "chat_id": "-100123",
                "sender": "alice",
                "text": "send email to admin@example.com about the project",
            }),
        ))
        .await
        .unwrap();
    assert!(
        dispatch.is_success(),
        "inject_inbound failed: {dispatch:#?}"
    );

    // The server emits the observation notification before its response, so
    // by the time dispatch returns the observation is already in the
    // pipeline; drain it (bounded defensively).
    let observations =
        wait_for_observations(bootstrap.manager(), std::time::Duration::from_secs(2)).await;
    let observation = observations
        .iter()
        .find(|o| o.kind == ObservationKind::InboundMessage)
        .expect("expected an inbound_message observation");
    assert_eq!(observation.source, CapabilityId::new("telegram.receive"));
    assert_eq!(observation.channel_id.as_deref(), Some("-100123"));
    assert_eq!(observation.sender.as_deref(), Some("alice"));
    assert_eq!(
        observation.payload["text"],
        "send email to admin@example.com about the project"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn mcp_runtime_config_helper_points_at_real_binary() {
    let root = temp_root("helper");
    let config = mcp_runtime_config("mcp", &["email"], &root);
    let (command, args) = match &config.transport {
        ai_os_openclaw_runtime::TransportConfig::Stdio { command, args } => (command, args),
    };
    assert!(command.contains("ai-os-mcp-server"));
    assert!(args.contains(&"--plugins".to_string()));
    assert!(args.iter().any(|a| a == "email"));
    let _ = std::fs::remove_dir_all(&root);
}
