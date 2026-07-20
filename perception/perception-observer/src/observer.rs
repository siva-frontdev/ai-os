use async_trait::async_trait;
use perception_core::{Observation, ObservationPriority, ObservationSource, ObserverKind};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use tokio::sync::mpsc;

pub type ObserverResult<T> = Result<T, ObserverError>;

#[derive(Debug, thiserror::Error)]
pub enum ObserverError {
    #[error("observer start failed: {0}")]
    StartFailed(String),
    #[error("observer stop failed: {0}")]
    StopFailed(String),
    #[error("observer not found: {0}")]
    NotFound(String),
    #[error("observer already started: {0}")]
    AlreadyStarted(String),
    #[error("observer not started: {0}")]
    NotStarted(String),
}

// ── ObserverConfig ────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObserverConfig {
    pub observer_id: String,
    pub kind: ObserverKind,
    pub enabled: bool,
    pub buffer_size: usize,
    pub priority: ObservationPriority,
}

impl Default for ObserverConfig {
    fn default() -> Self {
        Self {
            observer_id: String::new(),
            kind: ObserverKind::FileSystem,
            enabled: true,
            buffer_size: 1024,
            priority: ObservationPriority::Normal,
        }
    }
}

// ── ObserverStatus ────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObserverStatus {
    Initialized,
    Running,
    Draining,
    Stopped,
    Faulted,
}

// ── Observer trait ────────────────────────────────────────

#[async_trait]
pub trait Observer: Debug + Send + Sync {
    fn observer_id(&self) -> &str;

    fn kind(&self) -> &ObserverKind;

    fn config(&self) -> &ObserverConfig;

    async fn start(&self, tx: mpsc::Sender<Observation>) -> ObserverResult<()>;

    async fn stop(&self) -> ObserverResult<()>;

    fn status(&self) -> ObserverStatus;
}

// ── DefaultObserverBuilder ────────────────────────────────

pub struct DefaultObserverBuilder {
    config: ObserverConfig,
    source: ObservationSource,
}

impl DefaultObserverBuilder {
    pub fn new(config: ObserverConfig) -> Self {
        let source = ObservationSource {
            observer_id: config.observer_id.clone(),
            observer_kind: config.kind.clone(),
            instance_id: format!("inst-{}", uuid::Uuid::new_v4()),
            hostname: hostname(),
        };
        Self { config, source }
    }

    pub fn build(self) -> DefaultObserver {
        DefaultObserver {
            config: self.config,
            source: self.source,
            status: std::sync::Arc::new(std::sync::atomic::AtomicU8::new(0)),
        }
    }
}

fn hostname() -> String {
    std::env::var("HOSTNAME").unwrap_or_else(|_| "localhost".into())
}

// ── DefaultObserver ───────────────────────────────────────

#[derive(Debug)]
pub struct DefaultObserver {
    config: ObserverConfig,
    source: ObservationSource,
    status: std::sync::Arc<std::sync::atomic::AtomicU8>,
}

impl DefaultObserver {
    fn status_from_atomic(&self) -> ObserverStatus {
        match self.status.load(std::sync::atomic::Ordering::Relaxed) {
            0 => ObserverStatus::Initialized,
            1 => ObserverStatus::Running,
            2 => ObserverStatus::Draining,
            3 => ObserverStatus::Stopped,
            _ => ObserverStatus::Faulted,
        }
    }

    fn set_status(&self, status: ObserverStatus) {
        let val = match status {
            ObserverStatus::Initialized => 0,
            ObserverStatus::Running => 1,
            ObserverStatus::Draining => 2,
            ObserverStatus::Stopped => 3,
            ObserverStatus::Faulted => 4,
        };
        self.status.store(val, std::sync::atomic::Ordering::Relaxed);
    }
}

#[async_trait]
impl Observer for DefaultObserver {
    fn observer_id(&self) -> &str {
        &self.config.observer_id
    }

    fn kind(&self) -> &ObserverKind {
        &self.config.kind
    }

    fn config(&self) -> &ObserverConfig {
        &self.config
    }

    async fn start(&self, _tx: mpsc::Sender<Observation>) -> ObserverResult<()> {
        if self.status_from_atomic() == ObserverStatus::Running {
            return Err(ObserverError::AlreadyStarted(
                self.config.observer_id.clone(),
            ));
        }
        self.set_status(ObserverStatus::Running);
        Ok(())
    }

    async fn stop(&self) -> ObserverResult<()> {
        if self.status_from_atomic() != ObserverStatus::Running {
            return Err(ObserverError::NotStarted(self.config.observer_id.clone()));
        }
        self.set_status(ObserverStatus::Stopped);
        Ok(())
    }

    fn status(&self) -> ObserverStatus {
        self.status_from_atomic()
    }
}

// ── Default observers ─────────────────────────────────────

pub struct FileSystemObserver;

impl FileSystemObserver {
    pub fn new(config: ObserverConfig) -> DefaultObserver {
        DefaultObserverBuilder::new(config).build()
    }
}

pub struct ProcessObserver;

impl ProcessObserver {
    pub fn new(config: ObserverConfig) -> DefaultObserver {
        DefaultObserverBuilder::new(config).build()
    }
}

pub struct TerminalObserver;

impl TerminalObserver {
    pub fn new(config: ObserverConfig) -> DefaultObserver {
        DefaultObserverBuilder::new(config).build()
    }
}

pub struct NetworkObserver;

impl NetworkObserver {
    pub fn new(config: ObserverConfig) -> DefaultObserver {
        DefaultObserverBuilder::new(config).build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_observer_lifecycle() {
        let config = ObserverConfig {
            observer_id: "test-obs".into(),
            kind: ObserverKind::FileSystem,
            enabled: true,
            buffer_size: 100,
            priority: ObservationPriority::Normal,
        };
        let observer = FileSystemObserver::new(config);
        let (tx, _rx) = mpsc::channel(100);

        assert_eq!(observer.status(), ObserverStatus::Initialized);

        observer.start(tx).await.unwrap();
        assert_eq!(observer.status(), ObserverStatus::Running);

        // Double start should fail
        let (tx2, _rx2) = mpsc::channel(100);
        assert!(observer.start(tx2).await.is_err());

        observer.stop().await.unwrap();
        assert_eq!(observer.status(), ObserverStatus::Stopped);

        // Double stop should fail
        assert!(observer.stop().await.is_err());
    }

    #[tokio::test]
    async fn test_observer_identity() {
        let config = ObserverConfig {
            observer_id: "my-observer".into(),
            kind: ObserverKind::Terminal,
            enabled: true,
            buffer_size: 100,
            priority: ObservationPriority::Critical,
        };
        let observer = TerminalObserver::new(config);

        assert_eq!(observer.observer_id(), "my-observer");
        assert_eq!(*observer.kind(), ObserverKind::Terminal);
        assert_eq!(observer.config().priority, ObservationPriority::Critical);
    }
}
