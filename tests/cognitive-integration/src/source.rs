//! Observation source that polls the RuntimeManager for runtime observations.

use std::sync::Arc;

use ai_os_runtime_manager::RuntimeManager;
use async_trait::async_trait;
use brain_coordinator::companion_host::sources::ObservationSource;

/// Polls the [`RuntimeManager`] for observations from all registered runtimes.
///
/// Each poll cycle drains all available observations from the manager's
/// pull-based observation model and returns a summary string. The cognitive
/// loop's observation loop consumes this string.
pub struct RuntimeObservationSource {
    manager: Arc<RuntimeManager>,
    last_count: Arc<std::sync::atomic::AtomicUsize>,
}

impl RuntimeObservationSource {
    pub fn new(manager: Arc<RuntimeManager>) -> Self {
        Self {
            manager,
            last_count: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        }
    }

    /// The number of observations collected on the most recent poll.
    pub fn last_count(&self) -> usize {
        self.last_count.load(std::sync::atomic::Ordering::Relaxed)
    }
}

impl std::fmt::Debug for RuntimeObservationSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RuntimeObservationSource")
            .field("last_count", &self.last_count())
            .finish()
    }
}

#[async_trait]
impl ObservationSource for RuntimeObservationSource {
    fn name(&self) -> &'static str {
        "runtime"
    }

    async fn poll(&self) -> Option<String> {
        let observations = self.manager.observe().await;

        if observations.is_empty() {
            return None;
        }

        self.last_count
            .store(observations.len(), std::sync::atomic::Ordering::Relaxed);

        Some(format!(
            "runtime observations: {} new event(s)",
            observations.len()
        ))
    }
}
