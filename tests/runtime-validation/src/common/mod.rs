//! Shared harness for the runtime validation tests.

pub mod binary;
pub mod coordinator;
pub mod transport;

pub use binary::mcp_server_bin;
pub use coordinator::{scripted_understanding, ScriptedCoordinator};
pub use transport::{spawn_mcp_server, TestChildTransport};

use std::path::PathBuf;
use std::sync::Arc;

use ai_os_openclaw_runtime::OpenClawRuntimeConfig;
use ai_os_runtime_api::{Action, ActionResult, CapabilityId, RuntimeError};
use ai_os_runtime_manager::RuntimeManager;
use brain_coordinator::planner::{Plan, PlannedAction};

/// Translate a [`PlannedAction`] into a runtime [`Action`].
///
/// The same execution-layer routing used by the desktop companion: the
/// Planner emits capabilities, this wiring turns them into runtime actions.
pub fn action_from_planned(planned: &PlannedAction) -> Action {
    let input = match planned.action_type.as_str() {
        "email.send" => serde_json::json!({
            "to": planned.topics.first().cloned().unwrap_or_default(),
            "subject": planned.topics.get(1).cloned().unwrap_or_default(),
            "body": planned.topics.get(2).cloned().unwrap_or_default(),
        }),
        _ => serde_json::json!({
            "topics": planned.topics,
            "reason": planned.reason,
        }),
    };
    Action {
        action_id: uuid::Uuid::new_v4().to_string(),
        capability: CapabilityId::new(planned.action_type.clone()),
        input,
        trace_id: None,
        deadline_ms: None,
        metadata: Default::default(),
    }
}

/// Dispatch every runtime capability in a plan through the manager.
///
/// Brain capabilities are skipped — they are handled by cognition, not by
/// a runtime.
pub async fn dispatch_plan(
    manager: &RuntimeManager,
    plan: &Plan,
) -> Vec<Result<ActionResult, RuntimeError>> {
    let mut results = Vec::new();
    for planned in &plan.actions {
        let capability = CapabilityId::new(planned.action_type.clone());
        if !manager.has_capability(&capability).await {
            continue;
        }
        results.push(manager.dispatch(action_from_planned(planned)).await);
    }
    results
}

/// A fresh temp directory to use as the filesystem plugin root.
pub fn temp_root(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "ai-os-runtime-validation-{tag}-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&dir).expect("create temp root");
    dir
}

/// Build an [`OpenClawRuntimeConfig`] pointing at the real MCP server
/// binary with the given plugins, rooted at `root`.
pub fn mcp_runtime_config(
    id: &str,
    plugins: &[&str],
    root: &std::path::Path,
) -> OpenClawRuntimeConfig {
    OpenClawRuntimeConfig {
        runtime_id: id.into(),
        transport: ai_os_openclaw_runtime::TransportConfig::Stdio {
            command: mcp_server_bin().display().to_string(),
            args: vec![
                "--plugins".into(),
                plugins.join(","),
                "--root".into(),
                root.display().to_string(),
            ],
        },
        ..Default::default()
    }
}

/// Wrap a transport in a runtime and return both, so tests can reach the
/// child process handle (e.g. to kill it).
pub fn runtime_with_transport(
    id: &str,
    transport: Arc<dyn ai_os_openclaw_runtime::mcp::McpTransport>,
) -> (
    ai_os_openclaw_runtime::OpenClawRuntime,
    Arc<dyn ai_os_openclaw_runtime::mcp::McpTransport>,
) {
    let config = OpenClawRuntimeConfig {
        runtime_id: id.into(),
        ..Default::default()
    };
    let runtime =
        ai_os_openclaw_runtime::OpenClawRuntime::from_transport(config, transport.clone());
    (runtime, transport)
}

/// Drain observations from the manager until at least one arrives, or fail
/// after a deadline.
///
/// The observation is guaranteed to reach the manager's pull drain once the
/// MCP server has sent its notification (unbounded channel, no drops), so
/// this loop always terminates in practice; the deadline is a defensive
/// bound, not the synchronization mechanism.
pub async fn wait_for_observations(
    manager: &RuntimeManager,
    deadline: std::time::Duration,
) -> Vec<ai_os_runtime_api::Observation> {
    let start = std::time::Instant::now();
    loop {
        let observations = manager.observe().await;
        if !observations.is_empty() {
            return observations;
        }
        if start.elapsed() > deadline {
            panic!(
                "no observations reached the manager within {deadline:?}; runtime layer is broken"
            );
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }
}
