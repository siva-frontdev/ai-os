//! The [`RuntimeBootstrap`]: register and start runtimes at platform
//! startup.

use std::sync::Arc;

use ai_os_openclaw_runtime::{OpenClawRuntime, OpenClawRuntimeConfig, TransportConfig};
use ai_os_runtime_api::{Capability, Runtime, RuntimeError, RuntimeHealth, RuntimeId};
use ai_os_runtime_manager::RuntimeManager;

use crate::config::{McpRuntimeConfig, RuntimeBootstrapConfig, RuntimeInstanceConfig};
use crate::error::BootstrapError;

/// The outcome of a successful [`RuntimeBootstrap::initialize`].
#[derive(Debug, Clone)]
pub struct StartupSummary {
    /// Ids of every registered runtime.
    pub runtimes: Vec<RuntimeId>,
    /// The merged, deduplicated capability set.
    pub capabilities: Vec<Capability>,
    /// Health of every registered runtime.
    pub health: Vec<(RuntimeId, RuntimeHealth)>,
}

/// Owns the runtime set for the platform.
///
/// Construction spawns every configured runtime (the MCP subprocess starts)
/// and keeps strong references; registration into the manager and the MCP
/// handshake + capability discovery happen in
/// [`RuntimeBootstrap::initialize`].
#[derive(Debug)]
pub struct RuntimeBootstrap {
    config: RuntimeBootstrapConfig,
    manager: RuntimeManager,
    runtimes: Vec<Arc<OpenClawRuntime>>,
}

impl RuntimeBootstrap {
    /// Build the runtime set from an empty manager.
    pub fn new(config: RuntimeBootstrapConfig) -> Self {
        let manager = RuntimeManager::new();
        let mut runtimes = Vec::new();
        for instance in &config.instances {
            match instance {
                RuntimeInstanceConfig::Mcp(mcp) => match build_openclaw(mcp) {
                    Ok(runtime) => runtimes.push(runtime),
                    Err(e) => {
                        tracing::warn!(runtime_id = %mcp.id, error = %e, "failed to spawn MCP runtime; skipping");
                    }
                },
            }
        }
        Self {
            config,
            manager,
            runtimes,
        }
    }

    /// Build the runtime set from environment configuration.
    ///
    /// When the environment disables runtimes, this yields an empty
    /// bootstrap that `initialize()`s cleanly.
    pub fn from_env() -> Self {
        Self::new(RuntimeBootstrapConfig::from_env())
    }

    /// The underlying runtime manager.
    ///
    /// This is the handle the rest of the platform uses to discover and
    /// dispatch capabilities and to collect observations.
    pub fn manager(&self) -> &RuntimeManager {
        &self.manager
    }

    /// The configuration this bootstrap was built from.
    pub fn config(&self) -> &RuntimeBootstrapConfig {
        &self.config
    }

    /// Register all runtimes and perform the MCP handshake + capability
    /// discovery.
    ///
    /// Idempotent at the manager level: re-running re-merges capabilities
    /// without re-initializing runtimes that are already up.
    pub async fn initialize(&self) -> Result<StartupSummary, BootstrapError> {
        let registered = self.manager.runtime_ids().await;
        for runtime in &self.runtimes {
            let id = runtime.id();
            if !registered.contains(&id) {
                self.manager.register(runtime.clone()).await?;
            }
        }

        let ids = self.manager.runtime_ids().await;
        if ids.is_empty() {
            tracing::info!("no runtimes configured; runtime layer empty");
            return Ok(StartupSummary {
                runtimes: vec![],
                capabilities: vec![],
                health: vec![],
            });
        }

        self.manager.initialize().await?;
        let capabilities = self.manager.capabilities().await;
        let health = self.manager.health().await;

        tracing::info!(
            runtimes = ids.len(),
            capabilities = capabilities.len(),
            "runtime layer initialized"
        );
        for (id, state) in &health {
            tracing::info!(runtime_id = %id, health = ?state, "runtime status");
        }
        for capability in &capabilities {
            tracing::debug!(capability = %capability.id, "runtime capability available");
        }

        Ok(StartupSummary {
            runtimes: ids,
            capabilities,
            health,
        })
    }
}

fn build_openclaw(config: &McpRuntimeConfig) -> Result<Arc<OpenClawRuntime>, RuntimeError> {
    let mut openclaw_config = OpenClawRuntimeConfig {
        runtime_id: config.id.clone(),
        transport: TransportConfig::Stdio {
            command: config.command.clone(),
            args: config.args.clone(),
        },
        tool_allowlist: None,
        capability_overrides: std::collections::HashMap::new(),
        side_effects: std::collections::HashMap::new(),
    };
    if let Some(tools) = &config.allow_tools {
        openclaw_config.tool_allowlist = Some(tools.iter().cloned().collect());
    }
    openclaw_config.capability_overrides = config.capability_overrides.clone();

    let runtime = OpenClawRuntime::new(openclaw_config).map_err(|e| e.to_runtime_error())?;
    Ok(Arc::new(runtime))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ai_os_openclaw_runtime::mcp::protocol::McpTool;
    use ai_os_openclaw_runtime::mock::MockMcpServer;
    use serde_json::json;

    /// Build an empty-config bootstrap: registers nothing, initializes
    /// cleanly.
    #[tokio::test]
    async fn empty_bootstrap_initializes_cleanly() {
        let bootstrap = RuntimeBootstrap::new(RuntimeBootstrapConfig::default());
        let summary = bootstrap.initialize().await.unwrap();
        assert!(summary.runtimes.is_empty());
        assert!(summary.capabilities.is_empty());
        assert!(summary.health.is_empty());
        assert!(bootstrap.manager().runtime_ids().await.is_empty());
    }

    /// A bootstrap over a mock MCP runtime discovers capabilities.
    ///
    /// The mock is used here (not the real binary) because this crate's
    /// unit tests must not depend on a built subprocess.
    #[tokio::test]
    async fn mcp_bootstrap_discovers_capabilities() {
        let (client, server) = MockMcpServer::transport_pair();
        MockMcpServer::new(vec![McpTool {
            name: "email.send".into(),
            description: "Send email".into(),
            input_schema: json!({"type": "object"}),
        }])
        .run(server);

        let runtime = OpenClawRuntime::from_transport(
            ai_os_openclaw_runtime::OpenClawRuntimeConfig::default(),
            client,
        );
        let manager = RuntimeManager::new();
        manager.register(Arc::new(runtime)).await.unwrap();

        // Reuse the manager directly (the bootstrap builder spawns real
        // subprocesses, which unit tests must avoid).
        manager.initialize().await.unwrap();
        let caps = manager.capabilities().await;
        assert!(caps.iter().any(|c| c.id == "email.send".into()));
    }
}
