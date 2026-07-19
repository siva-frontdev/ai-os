use async_trait::async_trait;
use memory_core::Timestamp;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::episode_store::EpisodeStore;
use crate::error::{EpisodicError, EpisodicResult};
use crate::event::EpisodicEvent;
use crate::timeline::Timeline;

/// Primary interface for episodic memory — recording, replaying, and querying
/// the temporal event stream.
#[async_trait]
pub trait EpisodicMemory: Send + Sync + std::fmt::Debug {
    /// Record a new episodic event and return its generated ID.
    async fn record(&self, event: EpisodicEvent) -> EpisodicResult<Uuid>;

    /// Recall a single event by ID.
    async fn recall(&self, id: &Uuid) -> EpisodicResult<Option<EpisodicEvent>>;

    /// Return all events for a given session, ordered by timestamp.
    async fn recall_by_session(&self, session_id: &Uuid) -> EpisodicResult<Vec<EpisodicEvent>>;

    /// Return events whose timestamp falls within [start_ns, end_ns].
    async fn recall_by_time_range(
        &self,
        start_ns: i64,
        end_ns: i64,
    ) -> EpisodicResult<Vec<EpisodicEvent>>;

    /// Replay the full event chain containing the given event ID.
    ///
    /// Follows `sequence_id` links to reconstruct the temporal chain.
    async fn replay(&self, id: &Uuid) -> EpisodicResult<Vec<EpisodicEvent>>;

    /// Return aggregate statistics about stored episodes.
    async fn stats(&self) -> EpisodicResult<EpisodicStats>;
}

/// Aggregate statistics for the episodic memory store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpisodicStats {
    /// Total number of events stored.
    pub total_events: u64,
    /// Total number of distinct sessions.
    pub total_sessions: u64,
    /// Earliest and latest timestamps (if any events exist).
    pub date_range: Option<(i64, i64)>,
}

/// In-memory episodic memory backed by `EpisodeStore` + `Timeline`.
#[derive(Debug, Default)]
pub struct InMemoryEpisodicMemory {
    store: crate::episode_store::InMemoryEpisodeStore,
    timeline: crate::timeline::InMemoryTimeline,
}

impl InMemoryEpisodicMemory {
    /// Create a new empty episodic memory.
    pub fn new() -> Self {
        Self {
            store: crate::episode_store::InMemoryEpisodeStore::new(),
            timeline: crate::timeline::InMemoryTimeline::new(),
        }
    }
}

#[async_trait]
impl EpisodicMemory for InMemoryEpisodicMemory {
    async fn record(&self, event: EpisodicEvent) -> EpisodicResult<Uuid> {
        self.store.insert(event.clone()).await?;
        self.timeline.index(&event).await?;
        Ok(event.id)
    }
    async fn recall(&self, id: &Uuid) -> EpisodicResult<Option<EpisodicEvent>> {
        self.store.get(id).await
    }
    async fn recall_by_session(&self, session_id: &Uuid) -> EpisodicResult<Vec<EpisodicEvent>> {
        self.store.list_by_session(session_id).await
    }
    async fn recall_by_time_range(
        &self,
        start_ns: i64,
        end_ns: i64,
    ) -> EpisodicResult<Vec<EpisodicEvent>> {
        self.store.list_by_time_range(start_ns, end_ns).await
    }
    async fn replay(&self, id: &Uuid) -> EpisodicResult<Vec<EpisodicEvent>> {
        let Some(ev) = self.store.get(id).await? else {
            return Ok(Vec::new());
        };
        self.timeline.index(&ev).await?;
        Ok(vec![ev])
    }
    async fn stats(&self) -> EpisodicResult<EpisodicStats> {
        Ok(EpisodicStats {
            total_events: self.store.count().await?,
            total_sessions: 0,
            date_range: None,
        })
    }
}

/// A no-op implementation that returns errors for all operations.
#[derive(Debug, Default)]
pub struct DefaultEpisodicMemory;

#[async_trait]
impl EpisodicMemory for DefaultEpisodicMemory {
    async fn record(&self, _event: EpisodicEvent) -> EpisodicResult<Uuid> {
        Err(EpisodicError::Internal(
            "DefaultEpisodicMemory not configured".into(),
        ))
    }
    async fn recall(&self, _id: &Uuid) -> EpisodicResult<Option<EpisodicEvent>> {
        Err(EpisodicError::Internal(
            "DefaultEpisodicMemory not configured".into(),
        ))
    }
    async fn recall_by_session(&self, _session_id: &Uuid) -> EpisodicResult<Vec<EpisodicEvent>> {
        Err(EpisodicError::Internal(
            "DefaultEpisodicMemory not configured".into(),
        ))
    }
    async fn recall_by_time_range(
        &self,
        _start: i64,
        _end: i64,
    ) -> EpisodicResult<Vec<EpisodicEvent>> {
        Err(EpisodicError::Internal(
            "DefaultEpisodicMemory not configured".into(),
        ))
    }
    async fn replay(&self, _id: &Uuid) -> EpisodicResult<Vec<EpisodicEvent>> {
        Err(EpisodicError::Internal(
            "DefaultEpisodicMemory not configured".into(),
        ))
    }
    async fn stats(&self) -> EpisodicResult<EpisodicStats> {
        Err(EpisodicError::Internal(
            "DefaultEpisodicMemory not configured".into(),
        ))
    }
}
