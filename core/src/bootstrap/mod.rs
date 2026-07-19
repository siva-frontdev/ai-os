//! # Bootstrap
//!
//! Wires together every core subsystem using a builder pattern.
use std::fmt::Debug;
use std::sync::Arc;

use crate::application::Application;
use crate::config::{ConfigProvider, InMemoryConfigProvider, LayeredConfigProvider};
use crate::error::CoreError;
use crate::events::{EventBus, InMemoryEventBus};
use crate::health::{DefaultHealthMonitor, HealthMonitor};
use crate::lifecycle::{DefaultLifecycleManager, LifecycleManager};
use crate::logging::{ConsoleSink, DefaultLogger, LogSink, Logger, NullLogger, TracingBridge};
use crate::registry::{DefaultServiceRegistry, ServiceRegistry};

#[derive(Debug)]
pub struct PlatformBuilder {
    config_providers: Vec<Box<dyn ConfigProvider>>,
    log_sinks: Vec<Arc<dyn LogSink>>,
    enable_tracing: bool,
    enable_console_sink: bool,
    _development_mode: bool,
}

impl Default for PlatformBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl PlatformBuilder {
    pub fn new() -> Self {
        Self {
            config_providers: Vec::new(),
            log_sinks: Vec::new(),
            enable_tracing: false,
            enable_console_sink: true,
            _development_mode: true,
        }
    }

    pub fn with_config_provider(mut self, provider: Box<dyn ConfigProvider>) -> Self {
        self.config_providers.push(provider);
        self
    }

    pub fn with_log_sink(mut self, sink: Arc<dyn LogSink>) -> Self {
        self.log_sinks.push(sink);
        self
    }

    pub fn with_tracing(mut self) -> Self {
        self.enable_tracing = true;
        self
    }

    pub fn without_console_sink(mut self) -> Self {
        self.enable_console_sink = false;
        self
    }

    pub async fn build(self) -> Result<Application, CoreError> {
        // 1. Configuration
        let mut layered = LayeredConfigProvider::new();
        if self.config_providers.is_empty() {
            layered = layered.push(Box::new(InMemoryConfigProvider::new()));
        } else {
            for provider in self.config_providers {
                layered = layered.push(provider);
            }
        }
        let config: Arc<dyn ConfigProvider> = Arc::new(layered);

        // 2. Log sinks
        let sinks: Vec<Arc<dyn LogSink>> = if self.log_sinks.is_empty() && !self.enable_console_sink
        {
            vec![]
        } else if self.log_sinks.is_empty() && self.enable_console_sink {
            vec![Arc::new(ConsoleSink::new())]
        } else {
            self.log_sinks
        };

        if self.enable_tracing {
            TracingBridge::init_global_subscriber();
        }

        let root_logger: Arc<dyn Logger> = if sinks.is_empty() && !self.enable_tracing {
            NullLogger::boxed()
        } else {
            let mut logger = DefaultLogger::new("ai-os");
            for sink in &sinks {
                logger = logger.with_sink(sink.clone());
            }
            if self.enable_tracing {
                logger = logger.with_sink(Arc::new(TracingBridge::new()));
            }
            Arc::new(logger)
        };

        // 3. Event bus
        let event_bus: Arc<dyn EventBus> = Arc::new(InMemoryEventBus::new());

        // 4. Lifecycle manager
        let lifecycle: Arc<dyn LifecycleManager> = Arc::new(DefaultLifecycleManager::new());

        // 5. Service registry
        let registry: Arc<dyn ServiceRegistry> = Arc::new(DefaultServiceRegistry::new());

        // 6. Health monitor
        let health: Arc<dyn HealthMonitor> = Arc::new(DefaultHealthMonitor::new());

        let log = root_logger.child("bootstrap");
        log.info("core subsystems initialised");

        Ok(Application {
            event_bus,
            lifecycle,
            registry,
            health,
            config,
            logger: root_logger,
            running: std::sync::atomic::AtomicBool::new(false),
        })
    }
}
