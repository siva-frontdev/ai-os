//! A thin brain-facing helper that owns a single runtime.

use std::sync::Arc;

use crate::action::Action;
use crate::action::ActionResult;
use crate::capability::CapabilityId;
use crate::error::RuntimeError;
use crate::runtime::Runtime;

/// Owns one [`Runtime`] and provides the convenience entry point for
/// cognition: dispatch by capability.
///
/// The cognitive core depends on this type (and the [`Runtime`] trait),
/// never on any runtime implementation.
#[derive(Debug)]
pub struct RuntimeDispatcher {
    runtime: Arc<dyn Runtime>,
}

impl RuntimeDispatcher {
    /// Wrap a runtime.
    pub fn new(runtime: Arc<dyn Runtime>) -> Self {
        Self { runtime }
    }

    /// The wrapped runtime.
    pub fn runtime(&self) -> &Arc<dyn Runtime> {
        &self.runtime
    }

    /// Execute a capability with structured input.
    ///
    /// Generates a correlation id if none is implied by the caller.
    /// Returns the terminal `ActionResult`; `Err` only on
    /// interface/transport failure.
    pub async fn dispatch(
        &self,
        capability: CapabilityId,
        input: serde_json::Value,
    ) -> Result<ActionResult, RuntimeError> {
        let action = Action {
            action_id: uuid::Uuid::new_v4().to_string(),
            capability,
            input,
            trace_id: None,
            deadline_ms: None,
            metadata: Default::default(),
        };
        self.runtime.execute(action).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::Capability;
    use crate::observation::Observation;
    use crate::runtime::{RuntimeEvent, RuntimeHealth, RuntimeId};

    #[derive(Debug)]
    struct StubRuntime {
        caps: Vec<Capability>,
    }

    #[async_trait::async_trait]
    impl Runtime for StubRuntime {
        fn id(&self) -> RuntimeId {
            RuntimeId("stub".into())
        }
        async fn initialize(&self) -> Result<(), RuntimeError> {
            Ok(())
        }
        async fn capabilities(&self) -> Vec<Capability> {
            self.caps.clone()
        }
        async fn capability(&self, id: &CapabilityId) -> Option<Capability> {
            self.caps.iter().find(|c| &c.id == id).cloned()
        }
        async fn execute(&self, action: Action) -> Result<ActionResult, RuntimeError> {
            Ok(ActionResult::succeeded(
                action.action_id,
                serde_json::json!({"echo": action.input}),
            ))
        }
        async fn observe(&self) -> Vec<Observation> {
            Vec::new()
        }
        async fn subscribe(
            &self,
        ) -> Result<tokio::sync::mpsc::Receiver<RuntimeEvent>, RuntimeError> {
            Err(RuntimeError::SubscriptionUnsupported)
        }
        async fn health(&self) -> RuntimeHealth {
            RuntimeHealth::Ready
        }
    }

    #[tokio::test]
    async fn test_dispatcher_dispatch_routes_to_runtime() {
        let rt = StubRuntime {
            caps: vec![Capability {
                id: CapabilityId::new("email.send"),
                name: "Send email".into(),
                description: "desc".into(),
                input_schema: serde_json::json!({"type": "object"}),
                output_schema: None,
                side_effects: vec![],
                metadata: Default::default(),
            }],
        };
        let dispatcher = RuntimeDispatcher::new(Arc::new(rt));
        let result = dispatcher
            .dispatch(
                CapabilityId::new("email.send"),
                serde_json::json!({"to": "a@b.c"}),
            )
            .await
            .unwrap();
        assert!(result.is_success());
        assert_eq!(result.output.unwrap()["echo"]["to"], "a@b.c");
    }
}
