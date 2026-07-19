//! # Lifecycle Management
//!
//! Every long-lived platform component implements [`Service`],
//! which exposes `start` / `stop` hooks and a current state.
//! The [`LifecycleManager`] orchestrates startup and shutdown
//! order.
//!
//! ## Design decisions
//!
//! * **State machine** — services transition
//!   `Created → Initializing → Running → Stopping → Stopped`.
//! * **Reverse-order shutdown** — `stop_all` reverses the start
//!   order so dependents stop before their dependencies.
//! * **Arc-wrapped** — [`ManagedService`] is stored behind
//!   `Arc` so the manager can cheaply clone service references
//!   out of the lock.
use std::fmt::Debug;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::error::CoreError;

// ── Service State ─────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceState {
    Created = 0,
    Initializing = 1,
    Running = 2,
    Stopping = 3,
    Stopped = 4,
    Failed = 5,
}

impl ServiceState {
    fn from_u8(v: u8) -> Self {
        match v {
            0 => ServiceState::Created,
            1 => ServiceState::Initializing,
            2 => ServiceState::Running,
            3 => ServiceState::Stopping,
            4 => ServiceState::Stopped,
            _ => ServiceState::Failed,
        }
    }
}

// ── Service trait ─────────────────────────────────────────

#[async_trait]
pub trait Service: Debug + Send + Sync + 'static {
    fn name(&self) -> &str;
    async fn start(&self) -> Result<(), CoreError>;
    async fn stop(&self) -> Result<(), CoreError>;
}

// ── Managed service (internal, Arc-friendly) ──────────────

#[derive(Debug)]
pub(crate) struct ManagedService {
    pub service: Arc<dyn Service>,
    pub state: Arc<AtomicU8>,
    pub started_at: RwLock<Option<DateTime<Utc>>>,
}

impl ManagedService {
    pub fn new(service: Arc<dyn Service>) -> Arc<Self> {
        Arc::new(Self {
            service,
            state: Arc::new(AtomicU8::new(ServiceState::Created as u8)),
            started_at: RwLock::new(None),
        })
    }

    pub fn current_state(&self) -> ServiceState {
        ServiceState::from_u8(self.state.load(Ordering::Acquire))
    }

    pub fn set_state(&self, s: ServiceState) {
        self.state.store(s as u8, Ordering::Release);
    }
}

// ── Service status ────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ServiceStatus {
    pub name: String,
    pub state: ServiceState,
    pub started_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
}

// ── LifecycleManager trait ────────────────────────────────

#[async_trait]
pub trait LifecycleManager: Debug + Send + Sync {
    async fn register(&self, service: Arc<dyn Service>) -> Result<(), CoreError>;
    async fn start_all(&self) -> Result<(), CoreError>;
    async fn stop_all(&self) -> Result<(), CoreError>;
    fn state(&self, name: &str) -> Option<ServiceState>;
    fn status(&self) -> Vec<ServiceStatus>;
    fn service_count(&self) -> usize;
}

// ── Implementation ────────────────────────────────────────

#[derive(Debug)]
pub struct DefaultLifecycleManager {
    services: RwLock<Vec<Arc<ManagedService>>>,
}

impl DefaultLifecycleManager {
    pub fn new() -> Self {
        Self {
            services: RwLock::new(Vec::new()),
        }
    }
}

impl Default for DefaultLifecycleManager {
    fn default() -> Self {
        Self::new()
    }
}

fn find_service<'a>(
    services: &'a [Arc<ManagedService>],
    name: &str,
) -> Option<&'a Arc<ManagedService>> {
    services.iter().find(|m| m.service.name() == name)
}

#[async_trait]
impl LifecycleManager for DefaultLifecycleManager {
    async fn register(&self, service: Arc<dyn Service>) -> Result<(), CoreError> {
        let mut guard = self.services.write().map_err(|_| CoreError::LockPoisoned)?;
        if guard.iter().any(|m| m.service.name() == service.name()) {
            return Err(CoreError::State {
                name: service.name().to_string(),
                detail: "already registered".into(),
            });
        }
        guard.push(ManagedService::new(service));
        Ok(())
    }

    async fn start_all(&self) -> Result<(), CoreError> {
        let managed = {
            let guard = self.services.read().map_err(|_| CoreError::LockPoisoned)?;
            guard.clone()
        };

        for m in &managed {
            let name = m.service.name().to_string();
            m.set_state(ServiceState::Initializing);

            match m.service.start().await {
                Ok(()) => {
                    m.set_state(ServiceState::Running);
                    *m.started_at.write().map_err(|_| CoreError::LockPoisoned)? = Some(Utc::now());
                }
                Err(e) => {
                    m.set_state(ServiceState::Failed);
                    return Err(CoreError::StartFailed {
                        name,
                        detail: e.to_string(),
                    });
                }
            }
        }
        Ok(())
    }

    async fn stop_all(&self) -> Result<(), CoreError> {
        let managed = {
            let guard = self.services.read().map_err(|_| CoreError::LockPoisoned)?;
            let mut svcs = guard.clone();
            svcs.reverse();
            svcs
        };

        for m in &managed {
            let name = m.service.name().to_string();
            m.set_state(ServiceState::Stopping);

            if let Err(e) = m.service.stop().await {
                m.set_state(ServiceState::Failed);
                return Err(CoreError::StopFailed {
                    name,
                    detail: e.to_string(),
                });
            }

            m.set_state(ServiceState::Stopped);
        }
        Ok(())
    }

    fn state(&self, name: &str) -> Option<ServiceState> {
        let guard = self.services.read().ok()?;
        find_service(&guard, name).map(|m| m.current_state())
    }

    fn status(&self) -> Vec<ServiceStatus> {
        let guard = match self.services.read() {
            Ok(g) => g,
            Err(_) => return vec![],
        };
        guard
            .iter()
            .map(|m| {
                let started = m.started_at.read().ok().and_then(|g| *g);
                ServiceStatus {
                    name: m.service.name().to_string(),
                    state: m.current_state(),
                    started_at: started,
                    error: None,
                }
            })
            .collect()
    }

    fn service_count(&self) -> usize {
        self.services.read().map(|g| g.len()).unwrap_or(0)
    }
}

// ── Tests ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct NoopService {
        name: String,
    }

    #[async_trait]
    impl Service for NoopService {
        fn name(&self) -> &str {
            &self.name
        }
        async fn start(&self) -> Result<(), CoreError> {
            Ok(())
        }
        async fn stop(&self) -> Result<(), CoreError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn register_and_start_one() {
        let manager = DefaultLifecycleManager::new();
        manager
            .register(Arc::new(NoopService {
                name: "test".into(),
            }))
            .await
            .unwrap();
        assert_eq!(manager.service_count(), 1);
        assert_eq!(manager.state("test"), Some(ServiceState::Created));

        manager.start_all().await.unwrap();
        assert_eq!(manager.state("test"), Some(ServiceState::Running));
    }

    #[tokio::test]
    async fn start_stop_cycle() {
        let manager = DefaultLifecycleManager::new();
        manager
            .register(Arc::new(NoopService {
                name: "svc".into(),
            }))
            .await
            .unwrap();
        manager.start_all().await.unwrap();
        assert_eq!(manager.state("svc"), Some(ServiceState::Running));
        manager.stop_all().await.unwrap();
        assert_eq!(manager.state("svc"), Some(ServiceState::Stopped));
    }

    #[tokio::test]
    async fn start_failure_propagates() {
        #[derive(Debug)]
        struct FailingService;

        #[async_trait]
        impl Service for FailingService {
            fn name(&self) -> &str {
                "failer"
            }
            async fn start(&self) -> Result<(), CoreError> {
                Err(CoreError::General("start failed".into()))
            }
            async fn stop(&self) -> Result<(), CoreError> {
                Ok(())
            }
        }

        let manager = DefaultLifecycleManager::new();
        manager.register(Arc::new(FailingService)).await.unwrap();
        let result = manager.start_all().await;
        assert!(result.is_err());
        assert_eq!(manager.state("failer"), Some(ServiceState::Failed));
    }

    #[tokio::test]
    async fn duplicate_registration_rejected() {
        let manager = DefaultLifecycleManager::new();
        manager
            .register(Arc::new(NoopService {
                name: "dup".into(),
            }))
            .await
            .unwrap();
        let result = manager
            .register(Arc::new(NoopService {
                name: "dup".into(),
            }))
            .await;
        assert!(result.is_err());
    }

    #[test]
    fn status_snapshot() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let manager = DefaultLifecycleManager::new();
            manager
                .register(Arc::new(NoopService {
                    name: "svc".into(),
                }))
                .await
                .unwrap();
            manager.start_all().await.unwrap();

            let statuses = manager.status();
            assert_eq!(statuses.len(), 1);
            assert_eq!(statuses[0].name, "svc");
            assert_eq!(statuses[0].state, ServiceState::Running);
            assert!(statuses[0].started_at.is_some());
        });
    }
}
