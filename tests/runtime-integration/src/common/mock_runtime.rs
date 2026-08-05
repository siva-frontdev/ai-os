//! A second runtime implementation used to prove runtime swap.

use std::sync::Arc;

use ai_os_runtime_api::{
    Action, ActionResult, Capability, CapabilityId, Observation, Runtime, RuntimeError,
    RuntimeEvent, RuntimeHealth, RuntimeId,
};
use async_trait::async_trait;
use tokio::sync::{broadcast, Mutex};

/// An in-process runtime with a fixed capability set.
///
/// Executes actions locally, records them, and fans observations out to
/// both `observe()` and `subscribe()`. Registered in the manager exactly
/// like the OpenClaw runtime — proving the manager is runtime-agnostic.
#[derive(Debug)]
pub struct MockRuntime {
    id: RuntimeId,
    capabilities: Vec<Capability>,
    executed: Arc<Mutex<Vec<String>>>,
    observation_tx: broadcast::Sender<Observation>,
    observation_drain: Mutex<broadcast::Receiver<Observation>>,
}

impl MockRuntime {
    /// Create a mock runtime with the given capabilities.
    pub fn with_capabilities(id: &str, capabilities: Vec<Capability>) -> Arc<Self> {
        let (tx, rx) = broadcast::channel(64);
        Arc::new(Self {
            id: RuntimeId(id.into()),
            capabilities,
            executed: Arc::new(Mutex::new(Vec::new())),
            observation_tx: tx,
            observation_drain: Mutex::new(rx),
        })
    }

    /// Capabilities that the email flow tests use.
    pub fn email_capabilities() -> Vec<Capability> {
        vec![Capability {
            id: CapabilityId::new("email.send"),
            name: "email.send".into(),
            description: "Send an email message".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "to": {"type": "string"},
                    "subject": {"type": "string"},
                    "body": {"type": "string"}
                },
                "required": ["to"]
            }),
            output_schema: None,
            side_effects: vec![],
            metadata: Default::default(),
        }]
    }

    /// Push an observation into the runtime's stream.
    pub async fn push_observation(&self, observation: Observation) {
        let _ = self.observation_tx.send(observation);
    }

    /// The capability ids that were executed, in order.
    pub async fn executed(&self) -> Vec<String> {
        self.executed.lock().await.clone()
    }
}

#[async_trait]
impl Runtime for MockRuntime {
    fn id(&self) -> RuntimeId {
        self.id.clone()
    }

    async fn initialize(&self) -> Result<(), RuntimeError> {
        Ok(())
    }

    async fn capabilities(&self) -> Vec<Capability> {
        self.capabilities.clone()
    }

    async fn capability(&self, id: &CapabilityId) -> Option<Capability> {
        self.capabilities.iter().find(|c| &c.id == id).cloned()
    }

    async fn execute(&self, action: Action) -> Result<ActionResult, RuntimeError> {
        self.executed
            .lock()
            .await
            .push(action.capability.as_str().to_string());
        let supported = self.capability(&action.capability).await.is_some();
        if !supported {
            return Ok(ActionResult::failed(
                action.action_id,
                "capability_not_found",
                format!(
                    "capability '{}' not available on mock runtime",
                    action.capability
                ),
                false,
            ));
        }
        Ok(ActionResult::succeeded(
            action.action_id,
            serde_json::json!({
                "runtime": "mock",
                "capability": action.capability.as_str(),
                "echo": action.input,
            }),
        ))
    }

    async fn observe(&self) -> Vec<Observation> {
        let mut drain = self.observation_drain.lock().await;
        let mut out = Vec::new();
        while let Ok(obs) = drain.try_recv() {
            out.push(obs);
        }
        out
    }

    async fn subscribe(&self) -> Result<tokio::sync::mpsc::Receiver<RuntimeEvent>, RuntimeError> {
        let (tx, rx) = tokio::sync::mpsc::channel(64);
        let mut source = self.observation_tx.subscribe();
        tokio::spawn(async move {
            while let Ok(obs) = source.recv().await {
                if tx.send(RuntimeEvent::Observation(obs)).await.is_err() {
                    break;
                }
            }
        });
        Ok(rx)
    }

    async fn health(&self) -> RuntimeHealth {
        RuntimeHealth::Ready
    }
}
