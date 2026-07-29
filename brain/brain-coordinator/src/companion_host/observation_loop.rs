use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use tokio::sync::mpsc;

use super::sources::{ChangeDetector, ObservationSource};

/// An observation event recorded for debug visibility.
#[derive(Debug, Clone, serde::Serialize)]
pub enum ObservationEvent {
    Observed {
        source: &'static str,
        summary: String,
    },
    Skipped {
        source: &'static str,
        reason: String,
    },
}

/// Debug snapshot of the observation loop state.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ObservationDebugInfo {
    pub total_observations: u64,
    pub total_skipped: u64,
    pub history: Vec<ObservationEvent>,
    pub sources: Vec<String>,
}

/// Configuration for the observation loop.
#[derive(Debug, Clone)]
pub struct ObservationConfig {
    pub interval_secs: u64,
    pub max_history: usize,
}

impl Default for ObservationConfig {
    fn default() -> Self {
        Self {
            interval_secs: 30,
            max_history: 100,
        }
    }
}

// ── Shared inner state ─────────────────────────────────────

#[derive(Debug)]
struct ObservationLoopInner {
    source_names: Mutex<Vec<String>>,
    detector: ChangeDetector,
    config: ObservationConfig,
    total_observations: AtomicU64,
    history: Mutex<Vec<ObservationEvent>>,
}

impl ObservationLoopInner {
    fn record_event(&self, event: ObservationEvent) {
        if let Ok(mut history) = self.history.lock() {
            history.push(event);
            while history.len() > self.config.max_history {
                history.remove(0);
            }
        }
    }
}

// ── Public handle ──────────────────────────────────────────

/// Runs a continuous observation loop in the background.
///
/// Created and stored by CompanionHost. The CompanionHost calls
/// `spawn()` to start the background task and can later inspect
/// state via `debug_info()`.
#[derive(Debug, Clone)]
pub struct ObservationLoop {
    inner: Arc<ObservationLoopInner>,
}

impl ObservationLoop {
    pub fn new(config: ObservationConfig) -> Self {
        Self {
            inner: Arc::new(ObservationLoopInner {
                source_names: Mutex::new(Vec::new()),
                detector: ChangeDetector::new(),
                config,
                total_observations: AtomicU64::new(0),
                history: Mutex::new(Vec::new()),
            }),
        }
    }

    /// Record a source name (for debug visibility).
    pub fn register(&self, name: &str) {
        if let Ok(mut names) = self.inner.source_names.lock() {
            names.push(name.to_string());
        }
    }

    /// Collect all registered sources and spawn the background loop.
    ///
    /// Returns the task handle and the collected sources (for the
    /// spawned task to use). The sources are moved into the task.
    pub fn spawn(
        &self,
        sources: Vec<Box<dyn ObservationSource>>,
        cognitive_tx: mpsc::UnboundedSender<String>,
        loop_rx: tokio::sync::watch::Receiver<bool>,
    ) -> tokio::task::JoinHandle<()> {
        let inner = self.inner.clone();
        tokio::spawn(async move { run_loop(inner, sources, cognitive_tx, loop_rx).await })
    }

    /// Return debug info for developer mode.
    pub fn debug_info(&self) -> ObservationDebugInfo {
        let inner = &self.inner;
        let history = inner
            .history
            .lock()
            .ok()
            .map(|h| h.clone())
            .unwrap_or_default();
        let source_names: Vec<String> = inner
            .source_names
            .lock()
            .ok()
            .map(|n| n.clone())
            .unwrap_or_default();
        ObservationDebugInfo {
            total_observations: inner.total_observations.load(Ordering::Relaxed),
            total_skipped: inner.detector.skipped_count(),
            history,
            sources: source_names,
        }
    }

    /// Access the change detector (for testing).
    pub fn detector(&self) -> &ChangeDetector {
        &self.inner.detector
    }
}

async fn run_loop(
    inner: Arc<ObservationLoopInner>,
    sources: Vec<Box<dyn ObservationSource>>,
    cognitive_tx: mpsc::UnboundedSender<String>,
    mut loop_rx: tokio::sync::watch::Receiver<bool>,
) {
    let mut interval = tokio::time::interval(Duration::from_secs(inner.config.interval_secs));
    interval.tick().await;

    loop {
        tokio::select! {
            _ = interval.tick() => {
                for source in &sources {
                    let name = source.name();
                    let observation = match source.poll().await {
                        Some(text) => text,
                        None => continue,
                    };

                    if !inner.detector.is_changed(name, &observation) {
                        inner.record_event(ObservationEvent::Skipped {
                            source: name,
                            reason: "unchanged content".into(),
                        });
                        continue;
                    }

                    inner.total_observations.fetch_add(1, Ordering::Relaxed);
                    inner.record_event(ObservationEvent::Observed {
                        source: name,
                        summary: observation.chars().take(80).collect(),
                    });

                    tracing::debug!(source = name, "observation");
                    if cognitive_tx.send(observation).is_err() {
                        tracing::warn!("cognitive channel closed, stopping observation loop");
                        return;
                    }
                }
            }
            _ = loop_rx.changed() => {
                if !*loop_rx.borrow() {
                    tracing::info!("observation loop stopping");
                    return;
                }
            }
        }
    }
}
