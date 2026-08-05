//! The [`OpenClawRuntime`]: a [`Runtime`] implementation over MCP.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use ai_os_runtime_api::{
    now_ms, validate, Action, ActionResult, Capability, CapabilityId, Observation, Runtime,
    RuntimeError, RuntimeEvent, RuntimeHealth, RuntimeId,
};
use tokio::sync::{broadcast, Mutex};
use tokio::task::JoinHandle;

use crate::action_map;
use crate::capability_map;
use crate::config::OpenClawRuntimeConfig;
use crate::error::OpenClawError;
use crate::mcp::client::McpClient;
use crate::mcp::transport::{ChildTransport, McpTransport};
use crate::observation_map;
use crate::result_map;

/// The observation fan-out buffer.
const OBSERVATION_BUFFER: usize = 512;

/// A [`Runtime`] implementation that drives OpenClaw (or any MCP server)
/// as an execution substrate.
///
/// - **capabilities** are discovered via MCP `tools/list`
/// - **actions** are executed via MCP `tools/call`
/// - **observations** are received via MCP `notifications/observation`
///
/// This struct contains no reasoning, no prompting, no memory, and no
/// planning. It only translates.
#[derive(Debug)]
pub struct OpenClawRuntime {
    id: RuntimeId,
    config: OpenClawRuntimeConfig,
    transport: Arc<dyn McpTransport>,
    client: Mutex<Option<Arc<McpClient>>>,
    capabilities_cache: Mutex<Option<Vec<Capability>>>,
    initialized: AtomicBool,
    observations: broadcast::Sender<Observation>,
    observation_drain: Mutex<broadcast::Receiver<Observation>>,
    forwarder: Mutex<Option<JoinHandle<()>>>,
}

impl OpenClawRuntime {
    /// Construct from config, spawning the MCP subprocess (stdio).
    pub fn new(config: OpenClawRuntimeConfig) -> Result<Self, OpenClawError> {
        let transport: Arc<dyn McpTransport> = match &config.transport {
            crate::config::TransportConfig::Stdio { command, args } => {
                Arc::new(ChildTransport::spawn(command, args)?)
            }
        };
        Ok(Self::from_transport(config, transport))
    }

    /// Construct over a pre-built transport (used by tests and advanced
    /// wiring).
    pub fn from_transport(config: OpenClawRuntimeConfig, transport: Arc<dyn McpTransport>) -> Self {
        let id = RuntimeId(config.runtime_id.clone());
        let (observations, observation_drain) = broadcast::channel(OBSERVATION_BUFFER);
        Self {
            id,
            config,
            transport,
            client: Mutex::new(None),
            capabilities_cache: Mutex::new(None),
            initialized: AtomicBool::new(false),
            observations,
            observation_drain: Mutex::new(observation_drain),
            forwarder: Mutex::new(None),
        }
    }

    async fn connected_client(&self) -> Result<Arc<McpClient>, RuntimeError> {
        self.client
            .lock()
            .await
            .clone()
            .ok_or_else(|| RuntimeError::NotInitialized(self.id.0.clone()))
    }

    async fn spawn_observation_forwarder(&self, client: Arc<McpClient>) {
        let mut raw_rx = client.take_observation_receiver().await;
        let observation_tx = self.observations.clone();
        let forwarder = tokio::spawn(async move {
            while let Some(raw) = raw_rx.recv().await {
                let observation = observation_map::observation_from_mcp(raw);
                if observation_tx.send(observation).is_err() {
                    break;
                }
            }
        });
        *self.forwarder.lock().await = Some(forwarder);
    }
}

#[async_trait::async_trait]
impl Runtime for OpenClawRuntime {
    fn id(&self) -> RuntimeId {
        self.id.clone()
    }

    async fn initialize(&self) -> Result<(), RuntimeError> {
        {
            let mut slot = self.client.lock().await;
            if slot.is_some() {
                return Ok(());
            }
            let client = McpClient::connect(self.transport.clone())
                .await
                .map_err(|e| RuntimeError::Transport(format!("MCP connect failed: {e}")))?;
            let capabilities = client
                .list_tools()
                .await
                .map_err(|e| RuntimeError::Transport(format!("tools/list failed: {e}")))?
                .iter()
                .filter_map(|tool| {
                    capability_map::capability_from_tool(&self.id, tool, &self.config)
                })
                .collect::<Vec<_>>();

            *self.capabilities_cache.lock().await = Some(capabilities);
            self.spawn_observation_forwarder(client.clone()).await;
            *slot = Some(client);
        }
        self.initialized.store(true, Ordering::SeqCst);
        tracing::info!(runtime_id = %self.id, "openclaw runtime initialized");
        Ok(())
    }

    async fn capabilities(&self) -> Vec<Capability> {
        let cache = self.capabilities_cache.lock().await;
        cache.clone().unwrap_or_default()
    }

    async fn capability(&self, id: &CapabilityId) -> Option<Capability> {
        self.capabilities().await.into_iter().find(|c| &c.id == id)
    }

    async fn execute(&self, action: Action) -> Result<ActionResult, RuntimeError> {
        let started_ms = now_ms();
        let client = self.connected_client().await?;

        let capability = self.capability(&action.capability).await;
        let Some(capability) = capability else {
            return Ok(ActionResult::failed(
                action.action_id.clone(),
                "capability_not_found",
                format!(
                    "capability '{}' is not available on runtime '{}'",
                    action.capability, self.id
                ),
                false,
            ));
        };

        if let Err(reason) = validate(&action.input, &capability.input_schema) {
            return Ok(ActionResult::failed(
                action.action_id.clone(),
                "invalid_input",
                reason,
                false,
            ));
        }

        let tool = capability_map::tool_name_for(&capability);
        let arguments = action_map::build_tool_arguments(&action.input);

        tracing::debug!(
            action_id = %action.action_id,
            capability = %action.capability,
            tool = %tool,
            "calling MCP tool"
        );

        let result = client
            .call_tool(&tool, arguments)
            .await
            .map_err(|e| RuntimeError::Transport(format!("tools/call '{tool}' failed: {e}")))?;

        Ok(result_map::result_to_action_result(
            &action,
            result,
            started_ms,
            now_ms(),
        ))
    }

    async fn observe(&self) -> Vec<Observation> {
        let mut drain = self.observation_drain.lock().await;
        let mut observations = Vec::new();
        while let Ok(observation) = drain.try_recv() {
            observations.push(observation);
        }
        observations
    }

    async fn subscribe(&self) -> Result<tokio::sync::mpsc::Receiver<RuntimeEvent>, RuntimeError> {
        let (tx, rx) = tokio::sync::mpsc::channel(OBSERVATION_BUFFER);
        let mut source = self.observations.subscribe();
        tokio::spawn(async move {
            while let Ok(observation) = source.recv().await {
                if tx
                    .send(RuntimeEvent::Observation(observation))
                    .await
                    .is_err()
                {
                    break;
                }
            }
        });
        Ok(rx)
    }

    async fn health(&self) -> RuntimeHealth {
        let client = self.client.lock().await.clone();
        match client {
            Some(client) => match client.ping().await {
                Ok(()) => RuntimeHealth::Ready,
                Err(e) => RuntimeHealth::Degraded {
                    reason: format!("ping failed: {e}"),
                },
            },
            None => RuntimeHealth::Unavailable {
                reason: "runtime not initialized".into(),
            },
        }
    }
}
