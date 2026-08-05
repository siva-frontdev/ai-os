//! The [`RuntimeManager`]: aggregate lifecycle, capability merge, and
//! capability-based dispatch across runtimes.

use std::collections::HashMap;
use std::sync::Arc;

use ai_os_runtime_api::{
    Action, ActionResult, Capability, CapabilityId, Observation, Runtime, RuntimeError,
    RuntimeEvent, RuntimeHealth, RuntimeId,
};
use tokio::sync::RwLock;

/// A registered runtime alongside its routing map.
#[derive(Debug)]
struct RegisteredRuntime {
    runtime: Arc<dyn Runtime>,
}

/// Coordinates multiple runtimes behind the [`Runtime`] abstraction.
///
/// The Planner and the rest of cognition depend on this manager, never on
/// a concrete runtime. Dispatching is **capability-based**: the manager
/// routes an action to whichever runtime declared that capability, so no
/// runtime-specific code lives in the Planner.
#[derive(Debug)]
pub struct RuntimeManager {
    runtimes: RwLock<Vec<RegisteredRuntime>>,
    routing: RwLock<HashMap<CapabilityId, RuntimeId>>,
    capabilities: RwLock<Vec<Capability>>,
    initialized: RwLock<bool>,
}

impl Default for RuntimeManager {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeManager {
    /// Create an empty manager.
    pub fn new() -> Self {
        Self {
            runtimes: RwLock::new(Vec::new()),
            routing: RwLock::new(HashMap::new()),
            capabilities: RwLock::new(Vec::new()),
            initialized: RwLock::new(false),
        }
    }

    /// Register a runtime. Duplicate runtime ids are rejected.
    ///
    /// The runtime is not initialized here; call [`RuntimeManager::initialize`]
    /// after registering all runtimes.
    pub async fn register(&self, runtime: Arc<dyn Runtime>) -> Result<(), RuntimeError> {
        let id = runtime.id();
        let mut runtimes = self.runtimes.write().await;
        if runtimes.iter().any(|r| r.runtime.id() == id) {
            return Err(RuntimeError::DuplicateRuntime(id.0));
        }
        tracing::info!(runtime_id = %id, "runtime registered");
        runtimes.push(RegisteredRuntime { runtime });
        Ok(())
    }

    /// Register several runtimes at once.
    pub async fn register_many(
        &self,
        runtimes: impl IntoIterator<Item = Arc<dyn Runtime>>,
    ) -> Result<(), RuntimeError> {
        for runtime in runtimes {
            self.register(runtime).await?;
        }
        Ok(())
    }

    /// Initialize every registered runtime and merge their capabilities.
    ///
    /// Idempotent: calling again re-merges capabilities without
    /// re-initializing runtimes that are already up.
    pub async fn initialize(&self) -> Result<(), RuntimeError> {
        let runtimes = self.runtimes.read().await;
        let mut routing = HashMap::new();
        let mut merged: Vec<Capability> = Vec::new();

        for registered in runtimes.iter() {
            registered.runtime.initialize().await?;
            for capability in registered.runtime.capabilities().await {
                if let std::collections::hash_map::Entry::Vacant(entry) =
                    routing.entry(capability.id.clone())
                {
                    entry.insert(registered.runtime.id());
                } else {
                    tracing::warn!(
                        capability = %capability.id,
                        "capability already claimed by another runtime; ignoring duplicate"
                    );
                    continue;
                }
                merged.push(capability);
            }
        }

        merged.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
        let capability_count = merged.len();
        *self.routing.write().await = routing;
        *self.capabilities.write().await = merged;
        *self.initialized.write().await = true;
        tracing::info!(
            capabilities = capability_count,
            "runtime manager initialized"
        );
        Ok(())
    }

    /// True once [`RuntimeManager::initialize`] has run successfully.
    pub async fn is_initialized(&self) -> bool {
        *self.initialized.read().await
    }

    /// The merged, deduplicated, sorted capability set from all runtimes.
    pub async fn capabilities(&self) -> Vec<Capability> {
        self.capabilities.read().await.clone()
    }

    /// Resolve a single capability by id from the merged set.
    pub async fn capability(&self, id: &CapabilityId) -> Option<Capability> {
        self.capabilities
            .read()
            .await
            .iter()
            .find(|c| &c.id == id)
            .cloned()
    }

    /// Whether any runtime claims this capability.
    pub async fn has_capability(&self, id: &CapabilityId) -> bool {
        self.routing.read().await.contains_key(id)
    }

    /// Merge the merged runtime capabilities into an external registry.
    ///
    /// The caller supplies a closure (for example wiring the capabilities
    /// into the brain's `CapabilityRegistry`). Returns how many
    /// capabilities were handed over. The manager stays free of any
    /// dependency on the brain.
    pub async fn merge_capabilities<F>(&self, mut register: F) -> usize
    where
        F: FnMut(&Capability),
    {
        let capabilities = self.capabilities().await;
        let count = capabilities.len();
        for capability in &capabilities {
            register(capability);
        }
        count
    }

    /// The ids of all registered runtimes.
    pub async fn runtime_ids(&self) -> Vec<RuntimeId> {
        self.runtimes
            .read()
            .await
            .iter()
            .map(|r| r.runtime.id())
            .collect()
    }

    /// Dispatch an action to the runtime that claims its capability.
    ///
    /// - Unknown capability → `Ok(ActionResult { status: Failed,
    ///   code: "capability_not_found" })` (never an `Err`, never a panic).
    /// - `Err` is reserved for interface/transport failures.
    pub async fn dispatch(&self, action: Action) -> Result<ActionResult, RuntimeError> {
        let routing = self.routing.read().await;
        let Some(runtime_id) = routing.get(&action.capability) else {
            tracing::warn!(capability = %action.capability, "no runtime claims capability");
            return Ok(ActionResult::failed(
                action.action_id,
                "capability_not_found",
                format!(
                    "capability '{}' is not available on any runtime",
                    action.capability
                ),
                false,
            ));
        };

        let runtimes = self.runtimes.read().await;
        let Some(registered) = runtimes.iter().find(|r| r.runtime.id() == *runtime_id) else {
            return Err(RuntimeError::Internal(format!(
                "runtime '{}' missing from registry",
                runtime_id
            )));
        };

        tracing::debug!(
            capability = %action.capability,
            runtime_id = %runtime_id,
            action_id = %action.action_id,
            "dispatching action"
        );
        registered.runtime.execute(action).await
    }

    /// Aggregate observations from all runtimes (pull model).
    pub async fn observe(&self) -> Vec<Observation> {
        let runtimes = self.runtimes.read().await;
        let mut observations = Vec::new();
        for registered in runtimes.iter() {
            observations.extend(registered.runtime.observe().await);
        }
        observations
    }

    /// Subscribe to events from one runtime, identified by id.
    pub async fn subscribe(
        &self,
        id: &RuntimeId,
    ) -> Result<tokio::sync::mpsc::Receiver<RuntimeEvent>, RuntimeError> {
        let runtimes = self.runtimes.read().await;
        let Some(registered) = runtimes.iter().find(|r| &r.runtime.id() == id) else {
            return Err(RuntimeError::UnknownRuntime(id.0.clone()));
        };
        registered.runtime.subscribe().await
    }

    /// Current health of every registered runtime.
    pub async fn health(&self) -> Vec<(RuntimeId, RuntimeHealth)> {
        let runtimes = self.runtimes.read().await;
        let mut out = Vec::with_capacity(runtimes.len());
        for registered in runtimes.iter() {
            let id = registered.runtime.id();
            let health = registered.runtime.health().await;
            out.push((id, health));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ai_os_runtime_api::{Action, Observation, RuntimeHealth, RuntimeId};
    use std::sync::Mutex;

    /// A configurable in-process runtime used to test the manager.
    #[derive(Debug)]
    struct TestRuntime {
        id: &'static str,
        capabilities: Vec<Capability>,
        executed: Mutex<Vec<String>>,
    }

    impl TestRuntime {
        fn with_capabilities(id: &'static str, capabilities: Vec<Capability>) -> Arc<Self> {
            Arc::new(Self {
                id,
                capabilities,
                executed: Mutex::new(Vec::new()),
            })
        }

        fn executed(&self) -> Vec<String> {
            self.executed.lock().unwrap().clone()
        }
    }

    #[async_trait::async_trait]
    impl Runtime for TestRuntime {
        fn id(&self) -> RuntimeId {
            RuntimeId(self.id.into())
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
                .unwrap()
                .push(action.capability.as_str().to_string());
            Ok(ActionResult::succeeded(
                action.action_id,
                serde_json::json!({"runtime": self.id}),
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

    fn cap(id: &str) -> Capability {
        Capability {
            id: CapabilityId::new(id),
            name: id.into(),
            description: id.into(),
            input_schema: serde_json::json!({"type": "object"}),
            output_schema: None,
            side_effects: vec![],
            metadata: Default::default(),
        }
    }

    fn action(capability: &str) -> Action {
        Action {
            action_id: uuid::Uuid::new_v4().to_string(),
            capability: CapabilityId::new(capability),
            input: serde_json::json!({}),
            trace_id: None,
            deadline_ms: None,
            metadata: Default::default(),
        }
    }

    #[tokio::test]
    async fn test_register_and_initialize_merges_capabilities() {
        let manager = RuntimeManager::new();
        let a =
            TestRuntime::with_capabilities("alpha", vec![cap("email.send"), cap("chat.respond")]);
        let b = TestRuntime::with_capabilities("beta", vec![cap("fs.read")]);
        manager.register(a).await.unwrap();
        manager.register(b).await.unwrap();
        manager.initialize().await.unwrap();

        assert!(manager.is_initialized().await);
        let ids = manager.capabilities().await;
        let names: Vec<&str> = ids.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(names, vec!["chat.respond", "email.send", "fs.read"]);
        assert!(manager.has_capability(&CapabilityId::new("fs.read")).await);
        assert!(manager
            .capability(&CapabilityId::new("email.send"))
            .await
            .is_some());
    }

    #[tokio::test]
    async fn test_register_rejects_duplicate_runtime_id() {
        let manager = RuntimeManager::new();
        manager
            .register(TestRuntime::with_capabilities("same", vec![]))
            .await
            .unwrap();
        assert!(matches!(
            manager
                .register(TestRuntime::with_capabilities("same", vec![]))
                .await,
            Err(RuntimeError::DuplicateRuntime(_))
        ));
    }

    #[tokio::test]
    async fn test_dispatch_routes_to_owning_runtime() {
        let manager = RuntimeManager::new();
        let a = TestRuntime::with_capabilities("alpha", vec![cap("email.send")]);
        let b = TestRuntime::with_capabilities("beta", vec![cap("fs.read")]);
        let b_handle = b.clone();
        manager.register(a).await.unwrap();
        manager.register(b).await.unwrap();
        manager.initialize().await.unwrap();

        let result = manager.dispatch(action("fs.read")).await.unwrap();
        assert!(result.is_success());
        assert_eq!(result.output.unwrap()["runtime"], "beta");
        assert_eq!(b_handle.executed(), vec!["fs.read"]);
    }

    #[tokio::test]
    async fn test_dispatch_unknown_capability_is_typed_failure() {
        let manager = RuntimeManager::new();
        manager
            .register(TestRuntime::with_capabilities(
                "alpha",
                vec![cap("email.send")],
            ))
            .await
            .unwrap();
        manager.initialize().await.unwrap();

        let result = manager.dispatch(action("does.not_exist")).await.unwrap();
        assert_eq!(result.status, ai_os_runtime_api::ActionStatus::Failed);
        assert_eq!(result.error_code(), Some("capability_not_found"));
    }

    #[tokio::test]
    async fn test_dispatch_before_initialize_returns_not_found() {
        let manager = RuntimeManager::new();
        manager
            .register(TestRuntime::with_capabilities(
                "alpha",
                vec![cap("email.send")],
            ))
            .await
            .unwrap();
        let result = manager.dispatch(action("email.send")).await.unwrap();
        assert_eq!(result.error_code(), Some("capability_not_found"));
    }

    #[tokio::test]
    async fn test_merge_capabilities_invokes_callback() {
        let manager = RuntimeManager::new();
        manager
            .register(TestRuntime::with_capabilities(
                "alpha",
                vec![cap("email.send"), cap("chat.respond")],
            ))
            .await
            .unwrap();
        manager.initialize().await.unwrap();

        let mut seen = Vec::new();
        let count = manager
            .merge_capabilities(|c| seen.push(c.id.clone()))
            .await;
        assert_eq!(count, 2);
        assert!(seen.contains(&CapabilityId::new("email.send")));
    }

    #[tokio::test]
    async fn test_health_reports_each_runtime() {
        let manager = RuntimeManager::new();
        manager
            .register(TestRuntime::with_capabilities("alpha", vec![]))
            .await
            .unwrap();
        manager.initialize().await.unwrap();
        let health = manager.health().await;
        assert_eq!(health.len(), 1);
        assert_eq!(health[0].0, RuntimeId("alpha".into()));
        assert_eq!(health[0].1, RuntimeHealth::Ready);
    }
}
