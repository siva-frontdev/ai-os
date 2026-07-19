//! # Runtime
//!
//! Top-level runtime that collects all subsystems into one
//! struct.  This is what consumers create and use.
//!
//! ## Lifecycle
//!
//! The runtime itself implements [`Service`] from Core so it
//! can be registered with the platform's [`LifecycleManager`].
//!
//! 1. `start()` — transitions state from `Created` → `Running`
//! 2. `stop()` — transitions `Running` → `Draining` → `Stopped`
use std::fmt::Debug;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;

use ai_os_core::error::CoreError;
use ai_os_core::events::EventBus;
use ai_os_core::lifecycle::Service;
use ai_os_core::logging::Logger;

use crate::context::ContextManager;
use crate::error::RuntimeError;
use crate::permission::PermissionChecker;
use crate::resource::ResourceManager;
use crate::scheduler::Scheduler;
use crate::session::SessionManager;
use crate::state::RuntimeStateMachine;
use crate::supervisor::Supervisor;
use crate::task::TaskManager;

/// Combined runtime subsystem.
#[derive(Debug)]
pub struct Runtime {
    pub scheduler: Arc<dyn Scheduler>,
    pub supervisor: Arc<dyn Supervisor>,
    pub session_manager: Arc<dyn SessionManager>,
    pub task_manager: Arc<dyn TaskManager>,
    pub context_manager: Arc<dyn ContextManager>,
    pub state: Arc<dyn RuntimeStateMachine>,
    pub resource_manager: Arc<dyn ResourceManager>,
    pub permission_checker: Arc<dyn PermissionChecker>,
    pub event_bus: Arc<dyn EventBus>,
    pub logger: Arc<dyn Logger>,
    pub name: &'static str,
}

impl Runtime {
    pub fn new(
        event_bus: Arc<dyn EventBus>,
        logger: Arc<dyn Logger>,
    ) -> Self {
        Self {
            scheduler: Arc::new(crate::scheduler::PriorityScheduler::new()),
            supervisor: Arc::new(crate::supervisor::DefaultSupervisor::new()),
            session_manager: Arc::new(crate::session::DefaultSessionManager::new()),
            task_manager: Arc::new(crate::task::DefaultTaskManager::new()),
            context_manager: Arc::new(crate::context::DefaultContextManager::new()),
            state: Arc::new(crate::state::DefaultRuntimeState::new()),
            resource_manager: Arc::new(crate::resource::DefaultResourceManager::new()),
            permission_checker: Arc::new(crate::permission::DefaultPermissionChecker::new()),
            event_bus,
            logger,
            name: "runtime",
        }
    }

    /// Publish a runtime event on the event bus.
    pub async fn publish_event<E: ai_os_core::events::Event>(
        &self,
        event: &E,
    ) -> Result<(), CoreError> {
        self.event_bus.publish_event(event).await
    }
}

#[async_trait]
impl Service for Runtime {
    fn name(&self) -> &str {
        self.name
    }

    async fn start(&self) -> Result<(), CoreError> {
        let log = self.logger.child("runtime");
        log.info("runtime starting...");

        self.state
            .transition(crate::state::RuntimePhase::Initializing)
            .map_err(|e| CoreError::General(e.to_string()))?;

        // Publish state change event
        let _ = self
            .publish_event(&crate::state::RuntimePhaseChanged {
                from: crate::state::RuntimePhase::Created,
                to: crate::state::RuntimePhase::Initializing,
            })
            .await;

        self.state
            .transition(crate::state::RuntimePhase::Running)
            .map_err(|e| CoreError::General(e.to_string()))?;

        let _ = self
            .publish_event(&crate::state::RuntimePhaseChanged {
                from: crate::state::RuntimePhase::Initializing,
                to: crate::state::RuntimePhase::Running,
            })
            .await;

        log.info("runtime started");
        Ok(())
    }

    async fn stop(&self) -> Result<(), CoreError> {
        let log = self.logger.child("runtime");
        log.info("runtime stopping...");

        self.state
            .transition(crate::state::RuntimePhase::Draining)
            .map_err(|e| CoreError::General(e.to_string()))?;

        self.state
            .transition(crate::state::RuntimePhase::Stopped)
            .map_err(|e| CoreError::General(e.to_string()))?;

        log.info("runtime stopped");
        Ok(())
    }
}

// ── Tests ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use ai_os_core::bootstrap::PlatformBuilder;

    #[tokio::test]
    async fn runtime_lifecycle_via_core() {
        let app = PlatformBuilder::new()
            .without_console_sink()
            .build()
            .await
            .unwrap();

        let rt = Runtime::new(
            app.event_bus().clone(),
            app.logger().clone(),
        );

        assert_eq!(rt.state.phase(), crate::state::RuntimePhase::Created);

        rt.start().await.unwrap();
        assert_eq!(rt.state.phase(), crate::state::RuntimePhase::Running);

        rt.stop().await.unwrap();
        assert_eq!(rt.state.phase(), crate::state::RuntimePhase::Stopped);
    }

    #[tokio::test]
    async fn runtime_integration_with_core_services() {
        let app = PlatformBuilder::new()
            .without_console_sink()
            .build()
            .await
            .unwrap();

        let rt = Runtime::new(
            app.event_bus().clone(),
            app.logger().clone(),
        );

        // Register runtime as a core lifecycle service
        app.lifecycle()
            .register(Arc::new(rt))
            .await
            .unwrap();

        app.run().await.unwrap();
        app.shutdown().await.unwrap();
    }

    #[test]
    fn all_subsystems_accessible() {
        let app = tokio::runtime::Runtime::new().unwrap().block_on(async {
            PlatformBuilder::new()
                .without_console_sink()
                .build()
                .await
                .unwrap()
        });

        let rt = Runtime::new(
            app.event_bus().clone(),
            app.logger().clone(),
        );

        assert!(rt.scheduler.queue_depth() == 0);
        assert!(rt.session_manager.session_count() == 0);
        assert!(rt.task_manager.task_count() == 0);
        assert!(rt.resource_manager.total_usage().memory_bytes == 0);
        assert!(rt.supervisor.list_supervised().is_empty());
    }
}
