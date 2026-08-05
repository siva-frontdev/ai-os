//! A no-op runtime used for testing and graceful degradation.

use crate::action::Action;
use crate::action::ActionResult;
use crate::capability::{Capability, CapabilityId};
use crate::error::RuntimeError;
use crate::observation::Observation;
use crate::runtime::{Runtime, RuntimeEvent, RuntimeHealth, RuntimeId};

/// A runtime with zero capabilities.
///
/// Used in tests and in configurations where no execution substrate is
/// configured. Every method is a safe no-op or returns a typed error —
/// it never panics.
#[derive(Debug)]
pub struct DefaultRuntime;

#[async_trait::async_trait]
impl Runtime for DefaultRuntime {
    fn id(&self) -> RuntimeId {
        RuntimeId("default".into())
    }

    async fn initialize(&self) -> Result<(), RuntimeError> {
        Ok(())
    }

    async fn capabilities(&self) -> Vec<Capability> {
        Vec::new()
    }

    async fn capability(&self, _id: &CapabilityId) -> Option<Capability> {
        None
    }

    async fn execute(&self, action: Action) -> Result<ActionResult, RuntimeError> {
        Err(RuntimeError::NotInitialized(format!(
            "default runtime cannot execute '{}'",
            action.capability
        )))
    }

    async fn observe(&self) -> Vec<Observation> {
        Vec::new()
    }

    async fn subscribe(&self) -> Result<tokio::sync::mpsc::Receiver<RuntimeEvent>, RuntimeError> {
        Err(RuntimeError::SubscriptionUnsupported)
    }

    async fn health(&self) -> RuntimeHealth {
        RuntimeHealth::Unavailable {
            reason: "default runtime".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_default_runtime_is_harmless() {
        let rt = DefaultRuntime;
        assert_eq!(rt.id(), RuntimeId("default".into()));
        assert!(rt.initialize().await.is_ok());
        assert!(rt.capabilities().await.is_empty());
        assert!(rt.observe().await.is_empty());
        assert_eq!(
            rt.health().await,
            RuntimeHealth::Unavailable {
                reason: "default runtime".into()
            }
        );
        assert!(matches!(
            rt.subscribe().await,
            Err(RuntimeError::SubscriptionUnsupported)
        ));
    }

    #[tokio::test]
    async fn test_default_runtime_rejects_execution() {
        let rt = DefaultRuntime;
        let action = Action {
            action_id: "a".into(),
            capability: CapabilityId::new("anything.go"),
            input: serde_json::json!({}),
            trace_id: None,
            deadline_ms: None,
            metadata: Default::default(),
        };
        assert!(matches!(
            rt.execute(action).await,
            Err(RuntimeError::NotInitialized(_))
        ));
    }
}
