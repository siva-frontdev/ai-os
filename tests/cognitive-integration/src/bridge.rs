//! Cognitive-runtime bridge: wires the cognitive loop to the real MCP runtime.

use std::sync::Arc;

use ai_os_openclaw_runtime::{OpenClawRuntime, OpenClawRuntimeConfig};
use ai_os_runtime_api::Runtime;
use ai_os_runtime_manager::RuntimeManager;

use crate::executor::RuntimeAwareExecutor;
use crate::source::RuntimeObservationSource;

/// The full cognitive-runtime integration.
///
/// Assembles the pieces produced by Phases 3 and 4 — the `RuntimeManager`
/// (with its real MCP subprocess), the `RuntimeAwareExecutor` (dispatches
/// plan actions through the runtime), and the `RuntimeObservationSource`
/// (polls runtime observations for the cognitive loop) — into a single
/// handle that the cognitive loop can drive.
///
/// ## Lifecycle
///
/// 1. Construct from an `OpenClawRuntimeConfig` (pointing at the real
///    `ai-os-mcp-server` binary).
/// 2. Call `initialize()` — this boots the real MCP subprocess, performs
///    the MCP handshake, and discovers all plugin capabilities.
/// 3. Pass `manager()`, `executor()`, and `observation_source()` to your
///    cognitive loop / observation loop.
pub struct CognitiveRuntimeBridge {
    manager: Arc<RuntimeManager>,
}

impl CognitiveRuntimeBridge {
    /// Create a bridge hosting the given runtime.
    pub fn with_runtime(runtime: Arc<dyn Runtime>) -> Self {
        let manager = Arc::new(RuntimeManager::new());
        let _ = manager.register(runtime);
        Self { manager }
    }

    /// Register a real MCP runtime built from config.
    pub async fn with_mcp_runtime(config: OpenClawRuntimeConfig) -> Self {
        let runtime =
            Arc::new(OpenClawRuntime::new(config).expect("failed to build OpenClawRuntime"));
        let manager = Arc::new(RuntimeManager::new());
        manager
            .register(runtime)
            .await
            .expect("register MCP runtime");
        Self { manager }
    }

    /// Initialize — perform MCP handshake + capability discovery.
    ///
    /// Returns the count of discovered capabilities.
    pub async fn initialize(&self) -> Result<usize, crate::IntegrationError> {
        self.manager
            .initialize()
            .await
            .map_err(|e| crate::IntegrationError::Runtime(e.to_string()))?;

        let caps = self.manager.capabilities().await;
        let count = caps.len();

        tracing::info!("CognitiveRuntimeBridge: {} capabilities discovered", count);
        Ok(count)
    }

    /// Access the underlying runtime manager (backed by the real subprocess).
    pub fn manager(&self) -> &Arc<RuntimeManager> {
        &self.manager
    }

    /// Create a runtime-aware executor that dispatches plan actions.
    pub fn executor(&self) -> RuntimeAwareExecutor {
        RuntimeAwareExecutor::new(self.manager.clone())
    }

    /// Create an observation source that polls the runtime manager.
    pub fn observation_source(&self) -> RuntimeObservationSource {
        RuntimeObservationSource::new(self.manager.clone())
    }

    /// Check if the bridge is fully initialized.
    pub async fn is_ready(&self) -> bool {
        self.manager.is_initialized().await
    }

    /// Number of capabilities available.
    pub async fn capability_count(&self) -> usize {
        self.manager.capabilities().await.len()
    }
}

impl std::fmt::Debug for CognitiveRuntimeBridge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CognitiveRuntimeBridge")
            .field("manager", &"<RuntimeManager>")
            .finish()
    }
}
