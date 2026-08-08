//! Integration test: runtime registration and capability discovery.
//!
//! Runtime Manager → OpenClaw Runtime → Mock MCP → capabilities discovered.

use ai_os_openclaw_runtime::McpTool;
use ai_os_runtime_api::{CapabilityId, RuntimeId};
use ai_os_runtime_manager::RuntimeManager;
use runtime_integration_tests::common::openclaw::{
    default_handler, email_send_tool, openclaw_runtime_with_handler,
};

#[tokio::test]
async fn manager_discovers_openclaw_capabilities_via_mcp() {
    let tools = vec![
        email_send_tool(),
        McpTool {
            name: "telegram.post_message".into(),
            description: "Post a message to a Telegram chat".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "chat_id": {"type": "string"},
                    "text": {"type": "string"}
                },
                "required": ["chat_id", "text"]
            }),
        },
    ];

    let runtime =
        openclaw_runtime_with_handler(tools.clone(), "email.send", default_handler()).await;

    let manager = RuntimeManager::new();
    manager.register(runtime).await.unwrap();
    manager.initialize().await.unwrap();

    assert_eq!(
        manager.runtime_ids().await,
        vec![RuntimeId("openclaw".into())]
    );
    assert!(manager.is_initialized().await);

    // Capabilities merged from the MCP tools/list response.
    assert!(
        manager
            .has_capability(&CapabilityId::new("email.send"))
            .await
    );
    assert!(
        manager
            .has_capability(&CapabilityId::new("telegram.post_message"))
            .await
    );

    let capabilities = manager.capabilities().await;
    let names: Vec<&str> = capabilities.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(names, vec!["email.send", "telegram.post_message"]);

    // Input schemas round-trip from the tool declarations.
    let email = manager
        .capability(&CapabilityId::new("email.send"))
        .await
        .expect("email.send capability present");
    assert_eq!(email.input_schema["required"][0], "to");
    assert_eq!(email.side_effects, vec![]);
}

#[tokio::test]
async fn runtime_capabilities_merge_into_brain_registry() {
    let runtime =
        openclaw_runtime_with_handler(vec![email_send_tool()], "email.send", default_handler())
            .await;

    let manager = RuntimeManager::new();
    manager.register(runtime).await.unwrap();
    manager.initialize().await.unwrap();

    // Merge the runtime capabilities into the brain's CapabilityRegistry
    // so the Planner can reason about what is actually executable.
    let mut brain_registry = brain_coordinator::planner::CapabilityRegistry::new();
    let merged = manager
        .merge_capabilities(|capability| {
            brain_registry.register(brain_coordinator::planner::Capability {
                name: capability.id.as_str().to_string(),
                description: capability.description.clone(),
            });
        })
        .await;

    assert_eq!(merged, 1);
    assert!(brain_registry.get("email.send").is_some());
}
