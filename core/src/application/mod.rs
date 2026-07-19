//! # Application
//!
//! Top-level platform handle returned by
//! [`PlatformBuilder::build`](crate::bootstrap::PlatformBuilder).
//!
//! ## Design decisions
//!
//! * **Thin facade** — delegates to subsystems wired during
//!   bootstrap.
//! * **Direct accessors** — every subsystem is exposed via an
//!   accessor so consumers don't need to couple to bootstrap.
use std::fmt::Debug;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tokio::signal;

use crate::config::ConfigProvider;
use crate::error::CoreError;
use crate::events::{EventBus, StartedEvent};
use crate::health::HealthMonitor;
use crate::lifecycle::LifecycleManager;
use crate::logging::Logger;
use crate::registry::ServiceRegistry;

/// The running platform.
#[derive(Debug)]
pub struct Application {
    pub(crate) event_bus: Arc<dyn EventBus>,
    pub(crate) lifecycle: Arc<dyn LifecycleManager>,
    pub(crate) registry: Arc<dyn ServiceRegistry>,
    pub(crate) health: Arc<dyn HealthMonitor>,
    pub(crate) config: Arc<dyn ConfigProvider>,
    pub(crate) logger: Arc<dyn Logger>,
    pub(crate) running: AtomicBool,
}

impl Application {
    pub fn event_bus(&self) -> &Arc<dyn EventBus> {
        &self.event_bus
    }

    pub fn lifecycle(&self) -> &Arc<dyn LifecycleManager> {
        &self.lifecycle
    }

    pub fn registry(&self) -> &Arc<dyn ServiceRegistry> {
        &self.registry
    }

    pub fn health(&self) -> &Arc<dyn HealthMonitor> {
        &self.health
    }

    pub fn config(&self) -> &Arc<dyn ConfigProvider> {
        &self.config
    }

    pub fn logger(&self) -> &Arc<dyn Logger> {
        &self.logger
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }

    /// Start all services and emit [`StartedEvent`].
    pub async fn run(&self) -> Result<(), CoreError> {
        let log = self.logger.child("application");
        log.info("starting platform...");

        self.lifecycle.start_all().await?;
        self.running.store(true, Ordering::Release);

        self.event_bus.publish_event(&StartedEvent).await?;
        log.info("platform started");
        Ok(())
    }

    /// Gracefully stop all services.
    pub async fn shutdown(&self) -> Result<(), CoreError> {
        let log = self.logger.child("application");
        log.info("shutting down platform...");

        self.running.store(false, Ordering::Release);
        self.lifecycle.stop_all().await?;

        log.info("platform stopped");
        Ok(())
    }

    /// Run until SIGINT/SIGTERM, then shut down.
    pub async fn run_until_signal(&self) -> Result<(), CoreError> {
        self.run().await?;
        signal::ctrl_c()
            .await
            .map_err(|e| CoreError::General(e.to_string()))?;
        self.shutdown().await
    }
}
