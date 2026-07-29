use crate::error::PerceptionResult;
use crate::types::*;
use async_trait::async_trait;
use memory_core::Timestamp;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Debug;
use tokio::sync::mpsc;

// ── ObservationProvider ───────────────────────────────────

#[async_trait]
pub trait ObservationProvider: Debug + Send + Sync {
    fn source(&self) -> ObservationSource;

    async fn publish(&self, observation: Observation) -> PerceptionResult<()>;

    async fn publish_batch(&self, observations: Vec<Observation>) -> PerceptionResult<()>;

    fn register_pipeline(&self, kind: ObserverKind, tx: mpsc::Sender<Observation>);
}

// ── ObservationStore ──────────────────────────────────────

#[async_trait]
pub trait ObservationStore: Debug + Send + Sync {
    async fn store(&self, observation: &Observation) -> PerceptionResult<()>;

    async fn store_batch(&self, observations: &[Observation]) -> PerceptionResult<()>;

    async fn get(&self, id: &ObservationId) -> PerceptionResult<Option<Observation>>;

    async fn query(&self, filter: &ObservationFilter) -> PerceptionResult<Vec<Observation>>;

    async fn prune(&self, older_than: Timestamp) -> PerceptionResult<u64>;
}

// ── AttentionFilter ───────────────────────────────────────

#[async_trait]
pub trait AttentionFilter: Debug + Send + Sync {
    async fn evaluate(&self, observation: &Observation)
        -> PerceptionResult<Option<AttentionScore>>;

    fn record(&self, observation: &Observation);

    fn reset_modality(&self, modality: &Modality);

    fn config(&self) -> &AttentionConfig;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttentionScore {
    pub score: f64,
    pub is_novel: bool,
    pub habituation_count: u32,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttentionConfig {
    pub global_threshold: f64,
    pub habituation_decay: f64,
    pub novelty_bonus: f64,
    pub per_modality: HashMap<Modality, PerModalityAttention>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerModalityAttention {
    pub threshold: f64,
    pub decay: f64,
}

impl Default for AttentionConfig {
    fn default() -> Self {
        Self {
            global_threshold: 0.3,
            habituation_decay: 0.95,
            novelty_bonus: 0.2,
            per_modality: HashMap::new(),
        }
    }
}

// ── DesktopObservationProvider ──────────────────────────

#[async_trait]
pub trait DesktopObservationProvider: Debug + Send + Sync {
    async fn observe(&self) -> PerceptionResult<DesktopState>;
}
