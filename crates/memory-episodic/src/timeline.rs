use std::collections::{BTreeMap, HashMap};
use std::sync::RwLock;
use async_trait::async_trait;
use uuid::Uuid;

use crate::error::{EpisodicError, EpisodicResult};
use crate::event::EpisodicEvent;

/// Time-range query interface for episodic events.
///
/// Backed by a `BTreeMap<i64, Vec<Uuid>>` (timestamp → event IDs) enabling
/// efficient range scans and chronological traversal.
#[async_trait]
pub trait Timeline: Send + Sync + std::fmt::Debug {
    async fn index(&self, event: &EpisodicEvent) -> EpisodicResult<()>;
    async fn range(&self, start_ns: i64, end_ns: i64) -> EpisodicResult<Vec<EpisodicEvent>>;
    async fn recent(&self, n: usize) -> EpisodicResult<Vec<EpisodicEvent>>;
    async fn oldest(&self, n: usize) -> EpisodicResult<Vec<EpisodicEvent>>;
    async fn remove(&self, id: &Uuid) -> EpisodicResult<()>;
    async fn rebuild(&self) -> EpisodicResult<()>;
    async fn len(&self) -> EpisodicResult<usize>;
}

/// In-memory timeline backed by `std::sync::RwLock<BTreeMap<i64, Vec<Uuid>>>`.
#[derive(Debug, Default)]
pub struct InMemoryTimeline {
    by_timestamp: RwLock<BTreeMap<i64, Vec<Uuid>>>,
    by_id: RwLock<HashMap<Uuid, EpisodicEvent>>,
}

impl InMemoryTimeline {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl Timeline for InMemoryTimeline {
    async fn index(&self, event: &EpisodicEvent) -> EpisodicResult<()> {
        self.by_timestamp
            .write()
            .map_err(|e| EpisodicError::Internal(e.to_string()))?
            .entry(event.timestamp)
            .or_default()
            .push(event.id);
        self.by_id
            .write()
            .map_err(|e| EpisodicError::Internal(e.to_string()))?
            .insert(event.id, event.clone());
        Ok(())
    }
    async fn range(&self, start_ns: i64, end_ns: i64) -> EpisodicResult<Vec<EpisodicEvent>> {
        let ts_map = self.by_timestamp
            .read()
            .map_err(|e| EpisodicError::Internal(e.to_string()))?;
        let id_map = self.by_id
            .read()
            .map_err(|e| EpisodicError::Internal(e.to_string()))?;
        let mut results = Vec::new();
        for (_ts, ids) in ts_map.range(start_ns..=end_ns) {
            for id in ids {
                if let Some(ev) = id_map.get(id) {
                    results.push(ev.clone());
                }
            }
        }
        Ok(results)
    }
    async fn recent(&self, n: usize) -> EpisodicResult<Vec<EpisodicEvent>> {
        let ts_map = self.by_timestamp
            .read()
            .map_err(|e| EpisodicError::Internal(e.to_string()))?;
        let id_map = self.by_id
            .read()
            .map_err(|e| EpisodicError::Internal(e.to_string()))?;
        let mut results = Vec::new();
        for (_ts, ids) in ts_map.iter().rev().take(n) {
            for id in ids {
                if let Some(ev) = id_map.get(id) {
                    results.push(ev.clone());
                }
            }
        }
        results.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        results.truncate(n);
        Ok(results)
    }
    async fn oldest(&self, n: usize) -> EpisodicResult<Vec<EpisodicEvent>> {
        let ts_map = self.by_timestamp
            .read()
            .map_err(|e| EpisodicError::Internal(e.to_string()))?;
        let id_map = self.by_id
            .read()
            .map_err(|e| EpisodicError::Internal(e.to_string()))?;
        let mut results = Vec::new();
        for (_ts, ids) in ts_map.iter().take(n) {
            for id in ids {
                if let Some(ev) = id_map.get(id) {
                    results.push(ev.clone());
                }
            }
        }
        Ok(results)
    }
    async fn remove(&self, id: &Uuid) -> EpisodicResult<()> {
        let mut ts_map = self.by_timestamp
            .write()
            .map_err(|e| EpisodicError::Internal(e.to_string()))?;
        for (_ts, ids) in ts_map.iter_mut() {
            ids.retain(|i| i != id);
        }
        self.by_id
            .write()
            .map_err(|e| EpisodicError::Internal(e.to_string()))?
            .remove(id);
        Ok(())
    }
    async fn rebuild(&self) -> EpisodicResult<()> {
        let id_map = self.by_id
            .read()
            .map_err(|e| EpisodicError::Internal(e.to_string()))?;
        drop(id_map); // release read lock
        let mut ts_map = self.by_timestamp
            .write()
            .map_err(|e| EpisodicError::Internal(e.to_string()))?;
        let id_map = self.by_id
            .read()
            .map_err(|e| EpisodicError::Internal(e.to_string()))?;
        ts_map.clear();
        for (id, ev) in id_map.iter() {
            ts_map.entry(ev.timestamp).or_default().push(*id);
        }
        Ok(())
    }
    async fn len(&self) -> EpisodicResult<usize> {
        Ok(self.by_id
            .read()
            .map_err(|e| EpisodicError::Internal(e.to_string()))?
            .len())
    }
}

/// No-op default implementation.
#[derive(Debug, Default)]
pub struct DefaultTimeline;

#[async_trait]
impl Timeline for DefaultTimeline {
    async fn index(&self, _: &EpisodicEvent) -> EpisodicResult<()> { Ok(()) }
    async fn range(&self, _: i64, _: i64) -> EpisodicResult<Vec<EpisodicEvent>> { Ok(Vec::new()) }
    async fn recent(&self, _: usize) -> EpisodicResult<Vec<EpisodicEvent>> { Ok(Vec::new()) }
    async fn oldest(&self, _: usize) -> EpisodicResult<Vec<EpisodicEvent>> { Ok(Vec::new()) }
    async fn remove(&self, _: &Uuid) -> EpisodicResult<()> { Ok(()) }
    async fn rebuild(&self) -> EpisodicResult<()> { Ok(()) }
    async fn len(&self) -> EpisodicResult<usize> { Ok(0) }
}
