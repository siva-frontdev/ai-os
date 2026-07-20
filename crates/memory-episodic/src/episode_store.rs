use std::collections::HashMap;
use std::sync::RwLock;
use uuid::Uuid;
use serde::{Serialize, Deserialize};
use async_trait::async_trait;
use crate::error::{EpisodicError, EpisodicResult};
use crate::event::EpisodicEvent;

/// Low-level storage trait for episodic events by ID.
#[async_trait]
pub trait EpisodeStore: Send + Sync + std::fmt::Debug {
    async fn insert(&self, event: EpisodicEvent) -> EpisodicResult<()>;
    async fn get(&self, id: &Uuid) -> EpisodicResult<Option<EpisodicEvent>>;
    async fn delete(&self, id: &Uuid) -> EpisodicResult<()>;
    async fn list_by_session(&self, session_id: &Uuid) -> EpisodicResult<Vec<EpisodicEvent>>;
    async fn list_by_time_range(&self, start_ns: i64, end_ns: i64) -> EpisodicResult<Vec<EpisodicEvent>>;
    async fn count(&self) -> EpisodicResult<u64>;
    async fn clear(&self) -> EpisodicResult<()>;
}

/// In-memory episode store backed by `std::sync::RwLock<HashMap<Uuid, EpisodicEvent>>`.
#[derive(Debug, Default)]
pub struct InMemoryEpisodeStore {
    events: RwLock<HashMap<Uuid, EpisodicEvent>>,
}

impl InMemoryEpisodeStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl EpisodeStore for InMemoryEpisodeStore {
    async fn insert(&self, event: EpisodicEvent) -> EpisodicResult<()> {
        self.events
            .write()
            .map_err(|e| EpisodicError::Internal(e.to_string()))?
            .insert(event.id, event);
        Ok(())
    }
    async fn get(&self, id: &Uuid) -> EpisodicResult<Option<EpisodicEvent>> {
        Ok(self.events
            .read()
            .map_err(|e| EpisodicError::Internal(e.to_string()))?
            .get(id)
            .cloned())
    }
    async fn delete(&self, id: &Uuid) -> EpisodicResult<()> {
        self.events
            .write()
            .map_err(|e| EpisodicError::Internal(e.to_string()))?
            .remove(id);
        Ok(())
    }
    async fn list_by_session(&self, session_id: &Uuid) -> EpisodicResult<Vec<EpisodicEvent>> {
        let mut results: Vec<_> = self.events
            .read()
            .map_err(|e| EpisodicError::Internal(e.to_string()))?
            .values()
            .filter(|e| e.session_id == *session_id)
            .cloned()
            .collect();
        results.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        Ok(results)
    }
    async fn list_by_time_range(&self, start_ns: i64, end_ns: i64) -> EpisodicResult<Vec<EpisodicEvent>> {
        let mut results: Vec<_> = self.events
            .read()
            .map_err(|e| EpisodicError::Internal(e.to_string()))?
            .values()
            .filter(|e| e.timestamp.as_nanos() >= start_ns && e.timestamp.as_nanos() <= end_ns)
            .cloned()
            .collect();
        results.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        Ok(results)
    }
    async fn count(&self) -> EpisodicResult<u64> {
        Ok(self.events
            .read()
            .map_err(|e| EpisodicError::Internal(e.to_string()))?
            .len() as u64)
    }
    async fn clear(&self) -> EpisodicResult<()> {
        self.events
            .write()
            .map_err(|e| EpisodicError::Internal(e.to_string()))?
            .clear();
        Ok(())
    }
}

/// No-op default implementation.
#[derive(Debug, Default)]
pub struct DefaultEpisodeStore;

#[async_trait]
impl EpisodeStore for DefaultEpisodeStore {
    async fn insert(&self, _: EpisodicEvent) -> EpisodicResult<()> { Ok(()) }
    async fn get(&self, _: &Uuid) -> EpisodicResult<Option<EpisodicEvent>> { Ok(None) }
    async fn delete(&self, _: &Uuid) -> EpisodicResult<()> { Ok(()) }
    async fn list_by_session(&self, _: &Uuid) -> EpisodicResult<Vec<EpisodicEvent>> { Ok(Vec::new()) }
    async fn list_by_time_range(&self, _: i64, _: i64) -> EpisodicResult<Vec<EpisodicEvent>> { Ok(Vec::new()) }
    async fn count(&self) -> EpisodicResult<u64> { Ok(0) }
    async fn clear(&self) -> EpisodicResult<()> { Ok(()) }
}
