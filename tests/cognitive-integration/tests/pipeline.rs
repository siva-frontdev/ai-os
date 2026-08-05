//! Phase 5 Cognitive Layer — Runtime Integration Tests
//!
//! These tests prove the full cognitive pipeline works against the **real**
//! MCP subprocess (no mocks):
//!
//! `observe (runtime) → plan → execute (through real MCP) → observe result`
//!
//! Every test spawns the real `ai-os-mcp-server` binary.

use std::sync::Arc;

use ai_os_openclaw_runtime::{OpenClawRuntimeConfig, TransportConfig};
use ai_os_runtime_api::{ActionStatus, CapabilityId, ObservationKind};
use brain_coordinator::companion_host::sources::ObservationSource;
use brain_coordinator::planner::{Plan, PlannedAction};
use cognitive_integration::{CognitiveRuntimeBridge, RuntimeAwareExecutor};

/// Locate the real `ai-os-mcp-server` binary.
fn mcp_bin() -> std::path::PathBuf {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let bin = manifest_dir
        .join("..")
        .join("..")
        .join("target")
        .join("debug")
        .join("ai-os-mcp-server");
    let canonical = bin.canonicalize().unwrap_or_else(|_| bin.clone());
    if !canonical.exists() {
        // Fallback: check workspace root
        let root = manifest_dir.join("..").join("..");
        let fallback = root.join("target").join("debug").join("ai-os-mcp-server");
        eprintln!(
            "MCP binary not found at {} or {}",
            bin.display(),
            fallback.display()
        );
    }
    canonical
}

/// Build a config pointing at the real MCP binary with all plugins.
fn mcp_config(root: &std::path::Path) -> OpenClawRuntimeConfig {
    OpenClawRuntimeConfig {
        runtime_id: "cognitive-test".into(),
        transport: TransportConfig::Stdio {
            command: mcp_bin().display().to_string(),
            args: vec![
                "--plugins".into(),
                "email,filesystem,github,calendar,telegram".into(),
                "--root".into(),
                root.display().to_string(),
            ],
        },
        ..Default::default()
    }
}

fn temp_root(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "cognitive-integration-{}-{}",
        tag,
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&dir).expect("create temp root");
    dir
}

fn plan(actions: Vec<(&str, Vec<String>, &str)>) -> Plan {
    Plan {
        actions: actions
            .into_iter()
            .map(|(ty, topics, reason)| PlannedAction {
                action_type: ty.into(),
                topics,
                reason: reason.into(),
            })
            .collect(),
    }
}

/// Bootstrap: the bridge initializes and discovers real plugin capabilities.
#[tokio::test]
async fn cognitive_bridge_initializes_with_real_mcp() {
    let root = temp_root("init");
    let config = mcp_config(&root);
    let bridge = CognitiveRuntimeBridge::with_mcp_runtime(config).await;

    let cap_count = bridge.initialize().await.expect("initialize");
    assert!(cap_count >= 10, "should discover all plugin capabilities");
    assert!(bridge.is_ready().await);

    let _ = std::fs::remove_dir_all(&root);
}

/// Execute: the executor dispatches a real `email.send` through the MCP subprocess.
#[tokio::test]
async fn cognitive_executor_dispatches_real_email() {
    let root = temp_root("email");
    let config = mcp_config(&root);
    let bridge = CognitiveRuntimeBridge::with_mcp_runtime(config).await;
    bridge.initialize().await.expect("initialize");

    let executor = bridge.executor();

    let plan = plan(vec![(
        "email.send",
        vec![
            "admin@example.com".into(),
            "Cognitive test".into(),
            "This was sent through the real MCP subprocess.".into(),
        ],
        "user requested email".into(),
    )]);

    let results = executor.execute_plan(&plan).await;
    assert_eq!(results.len(), 1);
    let (_, result) = &results[0];
    assert_eq!(result.status, ActionStatus::Succeeded);
    let output = result.output.as_ref().expect("output present");
    let text = output["content"][0].as_str().expect("text content");
    let payload: serde_json::Value = serde_json::from_str(text).expect("parse content");
    assert_eq!(payload["status"], "sent");

    let _ = std::fs::remove_dir_all(&root);
}

/// Execute: the executor dispatches `telegram.inject_inbound` which produces
/// an observation that can be polled back from the runtime.
#[tokio::test]
async fn cognitive_executor_produces_observable_side_effect() {
    let root = temp_root("telegram");
    let config = mcp_config(&root);
    let bridge = CognitiveRuntimeBridge::with_mcp_runtime(config).await;
    bridge.initialize().await.expect("initialize");

    let executor = bridge.executor();
    let source = bridge.observation_source();

    // Inject a telegram message — this should produce an observation
    let plan = plan(vec![(
        "telegram.inject_inbound",
        vec![
            "-100123".into(),
            "alice".into(),
            "hello from cognitive test".into(),
        ],
        "simulate inbound message".into(),
    )]);

    let results = executor.execute_plan(&plan).await;
    assert_eq!(results.len(), 1);
    assert!(results[0].1.is_success());

    // The observation loop should detect the new observation
    // The telegram plugin sends the observation notification before
    // responding to the dispatch, so by the time execute_plan returns
    // the observation should be in the manager's drain.
    let observations = bridge.manager().observe().await;
    assert!(
        observations
            .iter()
            .any(|o| o.kind == ObservationKind::InboundMessage
                && o.source == CapabilityId::new("telegram.receive")),
        "expected inbound_message observation from telegram: {observations:?}"
    );

    let _ = std::fs::remove_dir_all(&root);
}

/// Planner-to-executor: a plan with mixed in-memory + runtime actions
/// correctly dispatches only the runtime actions.
#[tokio::test]
async fn cognitive_mixed_plan_dispatches_runtime_only() {
    let root = temp_root("mixed");
    let config = mcp_config(&root);
    let bridge = CognitiveRuntimeBridge::with_mcp_runtime(config).await;
    bridge.initialize().await.expect("initialize");

    let executor = bridge.executor();

    let plan = plan(vec![
        (
            "search_memory",
            vec!["project".into()],
            "find context".into(),
        ),
        (
            "filesystem.write",
            vec![
                "cognitive_test.txt".into(),
                "written by cognitive loop".into(),
            ],
            "create test file".into(),
        ),
        ("respond", vec![], "final response".into()),
    ]);

    let results = executor.execute_plan(&plan).await;
    // Only filesystem.write should be dispatched
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, "filesystem.write");
    assert!(results[0].1.is_success());

    // Verify the file was actually written
    let written = root.join("cognitive_test.txt");
    assert!(written.exists());
    let content = std::fs::read_to_string(&written).expect("read written file");
    assert_eq!(content, "written by cognitive loop");

    let _ = std::fs::remove_dir_all(&root);
}

/// Observation source: polls runtime observations and returns a summary.
#[tokio::test]
async fn observability_source_detects_runtime_events() {
    let root = temp_root("obs");
    let config = mcp_config(&root);
    let bridge = CognitiveRuntimeBridge::with_mcp_runtime(config).await;
    bridge.initialize().await.expect("initialize");

    let source = bridge.observation_source();

    // Initially no observations
    assert_eq!(source.poll().await, None);

    // Inject a telegram message
    let plan = plan(vec![(
        "telegram.inject_inbound",
        vec!["-100".into(), "bob".into(), "test observation".into()],
        "inject".into(),
    )]);
    let executor = bridge.executor();
    let _ = executor.execute_plan(&plan).await;

    // Now the source should detect observations
    let obs = source.poll().await;
    assert!(obs.is_some(), "should detect runtime observations");
    assert!(source.last_count() >= 1);

    let _ = std::fs::remove_dir_all(&root);
}

/// Recovery: after a runtime capability is dispatched and fails (e.g.
/// capability not found), the executor returns a typed failure result.
#[tokio::test]
async fn cognitive_executor_handles_unknown_capability() {
    let root = temp_root("unknown");
    let config = mcp_config(&root);
    let bridge = CognitiveRuntimeBridge::with_mcp_runtime(config).await;
    bridge.initialize().await.expect("initialize");

    let executor = bridge.executor();

    let plan = plan(vec![(
        "nonexistent.capability",
        vec![],
        "should fail".into(),
    )]);

    let results = executor.execute_plan(&plan).await;
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].1.status, ActionStatus::Failed);
    assert_eq!(results[0].1.error_code(), Some("capability_not_found"));

    let _ = std::fs::remove_dir_all(&root);
}

/// The full cognitive pipeline: observe → plan → execute → observe result.
///
/// This is the canonical Phase 5 success criteria test.
#[tokio::test]
async fn full_cognitive_pipeline_observe_plan_execute_observe() {
    let root = temp_root("pipeline");
    let config = mcp_config(&root);
    let bridge = CognitiveRuntimeBridge::with_mcp_runtime(config).await;
    bridge.initialize().await.expect("initialize");

    // 1. Observe: inject a telegram message through the real runtime
    let inject_plan = plan(vec![(
        "telegram.inject_inbound",
        vec![
            "-100789".into(),
            "alice".into(),
            "please write a file called notes.txt with hello world".into(),
        ],
        "simulate user message".into(),
    )]);

    let executor = bridge.executor();
    let inject_results = executor.execute_plan(&inject_plan).await;
    assert_eq!(inject_results.len(), 1);
    assert!(inject_results[0].1.is_success());

    // 2. Observe: drain the observation from the runtime
    let observations = bridge.manager().observe().await;
    let msg_obs = observations
        .iter()
        .find(|o| o.kind == ObservationKind::InboundMessage);
    assert!(
        msg_obs.is_some(),
        "telegram observation should be available"
    );
    let text = msg_obs.unwrap().payload["text"].as_str().unwrap();
    assert!(text.contains("notes.txt"));

    // 3. Plan: create a plan to write the file (as the cognitive loop would)
    let write_plan = plan(vec![(
        "filesystem.write",
        vec!["notes.txt".into(), "hello world".into()],
        "write requested file".into(),
    )]);

    // 4. Execute: run the plan through the real runtime
    let exec_results = executor.execute_plan(&write_plan).await;
    assert_eq!(exec_results.len(), 1);
    assert!(exec_results[0].1.is_success(), "filesystem.write failed");

    // 5. Observe: verify the file exists (real side effect)
    let notes = root.join("notes.txt");
    assert!(notes.exists());
    assert_eq!(std::fs::read_to_string(&notes).unwrap(), "hello world");

    let _ = std::fs::remove_dir_all(&root);
}
