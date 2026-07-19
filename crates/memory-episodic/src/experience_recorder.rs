use std::sync::RwLock;
use async_trait::async_trait;
use uuid::Uuid;
use std::collections::HashMap;

use crate::error::{EpisodicError, EpisodicResult};
use crate::event::EpisodicEvent;
use crate::episode_store::EpisodeStore;
use crate::timeline::Timeline;

/// Records experiences into episodic memory with automatic importance scoring.
///
/// Consolidation policies determine when an experience is promoted to
/// episodic memory from working memory based on importance, recency,
/// repetition, and confidence.
#[async_trait]
pub trait ExperienceRecorder: Send + Sync + std::fmt::Debug {
    async fn record(&self, event: EpisodicEvent) -> EpisodicResult<Uuid>;
    async fn score_importance(&self, event: &EpisodicEvent) -> EpisodicResult<f32>;
    async fn should_consolidate(&self, event: &EpisodicEvent) -> EpisodicResult<bool>;
    async fn count(&self) -> EpisodicResult<u64>;
}

/// In-memory implementation backed by `EpisodeStore` + `Timeline`.
#[derive(Debug, Default)]
pub struct InMemoryExperienceRecorder {
    store: crate::episode_store::InMemoryEpisodeStore,
    timeline: crate::timeline::InMemoryTimeline,
    importance_threshold: RwLock<f32>,
    max_events: RwLock<Option<usize>>,
}

impl InMemoryExperienceRecorder {
    pub fn new() -> Self {
        Self {
            store: crate::episode_store::InMemoryEpisodeStore::new(),
            timeline: crate::timeline::InMemoryTimeline::new(),
            importance_threshold: RwLock::new(0.3),
            max_events: RwLock::new(None),
        }
    }

    /// Set the minimum importance threshold for consolidation.
    pub fn set_importance_threshold(&self, threshold: f32) {
        let mut guard = self.importance_threshold.write().unwrap();
        *guard = threshold.clamp(0.0, 1.0);
    }

    /// Set a maximum number of events to retain.
    pub fn set_max_events(&self, max: Option<usize>) {
        let mut guard = self.max_events.write().unwrap();
        *guard = max;
    }
}

#[async_trait]
impl ExperienceRecorder for InMemoryExperienceRecorder {
    async fn record(&self, event: EpisodicEvent) -> EpisodicResult<Uuid> {
        let id = event.id;
        self.store.insert(event.clone()).await?;
        self.timeline.index(&event).await?;
        Ok(id)
    }
    async fn score_importance(&self, event: &EpisodicEvent) -> EpisodicResult<f32> {
        let recency_boost = if event.timestamp > 0 {
            let now_ns = memory_core::Timestamp::now().as_nanos() as u64;
            let event_ns = event.timestamp as u64;
            let age_ns = now_ns.saturating_sub(event_ns);
            let hour_ns: u64 = 3_600_000_000_000;
            if age_ns < hour_ns { 0.1 } else { 0.0 }
        } else { 0.0 };
        let repetition_boost = if let Some(count_str) = event.metadata.get("access_count") {
            count_str.parse::<f32>().unwrap_or(0.0) * 0.01
        } else { 0.0 };
        Ok((event.importance + recency_boost + repetition_boost).clamp(0.0, 1.0))
    }
    async fn should_consolidate(&self, event: &EpisodicEvent) -> EpisodicResult<bool> {
        let score = self.score_importance(event).await?;
        let threshold = *self.importance_threshold.read().unwrap();
        Ok(score >= threshold)
    }
    async fn count(&self) -> EpisodicResult<u64> {
        self.store.count().await
    }
}

/// No-op default implementation.
#[derive(Debug, Default)]
pub struct DefaultExperienceRecorder;

#[async_trait]
impl ExperienceRecorder for DefaultExperienceRecorder {
    async fn record(&self, _: EpisodicEvent) -> EpisodicResult<Uuid> {
        Err(EpisodicError::Internal("DefaultExperienceRecorder not configured".into()))
    }
    async fn score_importance(&self, _: &EpisodicEvent) -> EpisodicResult<f32> { Ok(0.0) }
    async fn should_consolidate(&self, _: &EpisodicEvent) -> EpisodicResult<bool> { Ok(false) }
    async fn count(&self) -> EpisodicResult<u64> { Ok(0) }
}
