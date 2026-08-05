//! The E2E flow: an inbound runtime observation → the real Cognitive Loop
//! → the World Model.
//!
//! These tests prove the Phase 3 contract end to end with the real MCP
//! subprocess:
//!
//! ```text
//! telegram.inject_inbound (dispatch)
//!   → notifications/observation → OpenClawRuntime → RuntimeManager
//!   → CognitiveLoopService.cycle(text)          (scripted AI, real Rust)
//!   → EvolutionEngine → InMemoryWorldModelStore
//! ```

use std::collections::HashMap;
use std::sync::Arc;

use ai_os_openclaw_runtime::mcp::McpTransport;
use ai_os_runtime_api::{Action, CapabilityId, ObservationKind};
use ai_os_runtime_manager::RuntimeManager;
use brain_coordinator::cognitive_loop::CognitiveLoopService;
use brain_coordinator::planner::Planner;
use intelligence_coordinator::world_understanding::{StructuredWorldUpdate, WorldEntity};
use memory_storage::wm_store::{InMemoryWorldModelStore, WorldModelStore};
use runtime_validation_tests::common::{
    dispatch_plan, runtime_with_transport, scripted_understanding, spawn_mcp_server, temp_root,
    wait_for_observations, ScriptedCoordinator,
};

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

/// Start a real MCP subprocess hosting telegram + email, registered in a
/// fresh manager and initialized.
async fn manager_with_real_subprocess(root: &std::path::Path) -> RuntimeManager {
    let raw = spawn_mcp_server(&["telegram", "email"], root).expect("spawn real mcp server");
    let transport: Arc<dyn McpTransport> = raw.clone();
    let (runtime, _transport) = runtime_with_transport("mcp", transport);
    let manager = RuntimeManager::new();
    manager.register(Arc::new(runtime)).await.unwrap();
    manager.initialize().await.unwrap();
    manager
}

/// An understanding that extracts "Alice" as a person.
fn alice_understanding() -> StructuredWorldUpdate {
    StructuredWorldUpdate {
        entities: vec![WorldEntity {
            name: "Alice".into(),
            entity_type: "person".into(),
            properties: HashMap::new(),
            confidence: 0.9,
            importance: 0.7,
        }],
        relationships: vec![],
        state_changes: vec![],
        new_observations: vec![],
        open_questions: vec![],
        possible_hypotheses: vec![],
    }
}

#[tokio::test]
async fn inbound_telegram_observation_reaches_world_model() {
    let root = temp_root("wm");
    let manager = manager_with_real_subprocess(&root).await;

    // 1. An inbound telegram message arrives as a dispatch, and the runtime
    //    turns it into an observation.
    let injected = manager
        .dispatch(action(
            "telegram.inject_inbound",
            serde_json::json!({
                "chat_id": "-100123",
                "sender": "alice",
                "text": "my name is Alice and I work on AI-OS",
            }),
        ))
        .await
        .unwrap();
    assert!(
        injected.is_success(),
        "inject_inbound failed: {injected:#?}"
    );

    let observations = wait_for_observations(&manager, std::time::Duration::from_secs(2)).await;
    let observation = observations
        .iter()
        .find(|o| o.kind == ObservationKind::InboundMessage)
        .expect("expected an inbound_message observation");
    assert_eq!(observation.source, CapabilityId::new("telegram.receive"));
    assert_eq!(observation.sender.as_deref(), Some("alice"));
    let text = observation.payload["text"].as_str().unwrap().to_string();

    // 2. Feed the observation into the real Cognitive Loop. The AI
    //    coordinator is scripted; the whole Rust pipeline (understanding →
    //    memory evaluation → evolution → planning → execution) is real.
    let store = Arc::new(InMemoryWorldModelStore::new());
    let understanding = scripted_understanding(alice_understanding());
    let coordinator = ScriptedCoordinator::new(
        r#"{"actions":[{"type":"respond","reason":"acknowledge"}]}"#,
        r#"[{"store":true,"importance":0.5,"confidence":0.5,"reason":"durable person","summary":""}]"#,
        "Nice to meet you, Alice!",
    );
    let mut loop_svc = CognitiveLoopService::new(store.clone(), understanding, coordinator);
    let _decision = loop_svc.cycle(&text).await;

    // 3. The World Model absorbed the observation.
    let entities = store.all_entities().await;
    assert!(
        entities.iter().any(|e| e.name == "Alice"),
        "Alice should be in the World Model, got {entities:?}"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn inbound_telegram_message_plans_and_sends_email() {
    let root = temp_root("emailflow");
    let manager = manager_with_real_subprocess(&root).await;

    // 1. Inbound telegram message → observation.
    let injected = manager
        .dispatch(action(
            "telegram.inject_inbound",
            serde_json::json!({
                "chat_id": "-100999",
                "sender": "alice",
                "text": "send email to admin@example.com about the project",
            }),
        ))
        .await
        .unwrap();
    assert!(injected.is_success());
    let observations = wait_for_observations(&manager, std::time::Duration::from_secs(2)).await;
    let observation = observations
        .iter()
        .find(|o| o.kind == ObservationKind::InboundMessage)
        .expect("expected an inbound_message observation");
    let text = observation.payload["text"].as_str().unwrap().to_string();

    // 2. The (scripted) coordinator plans an email.send; the real Planner
    //    parses it.
    let coordinator = ScriptedCoordinator::new(
        r#"{"actions":[{"type":"email.send","topics":["admin@example.com","Project update","The project is on track."],"reason":"user requested an email"}]}"#,
        r#"[{"store":true,"importance":0.4,"confidence":0.5,"reason":"project context","summary":""}]"#,
        "Email sent successfully!",
    );
    let planner = Planner::new(coordinator);
    let plan = planner.plan(&text, "", "", false).await;
    assert_eq!(plan.actions[0].action_type, "email.send");

    // 3. The plan dispatches through the real MCP subprocess.
    let results = dispatch_plan(&manager, &plan).await;
    assert_eq!(results.len(), 1);
    let email_result = results[0]
        .as_ref()
        .expect("dispatch did not fail transport");
    assert!(
        email_result.is_success(),
        "email.send failed: {email_result:#?}"
    );
    let text = email_result.output.as_ref().expect("output present")["content"][0]
        .as_str()
        .unwrap();
    let payload: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(payload["status"], "sent");
    assert_eq!(payload["to"], "admin@example.com");

    // 4. The email plugin observed its own action (status_changed).
    let observations = wait_for_observations(&manager, std::time::Duration::from_secs(2)).await;
    assert!(
        observations.iter().any(|o| {
            o.source == CapabilityId::new("email.send") && o.kind == ObservationKind::StatusChanged
        }),
        "expected a status_changed observation from email.send: {observations:#?}"
    );

    let _ = std::fs::remove_dir_all(&root);
}
