use async_trait::async_trait;
use memory_core::{MemoryObject, MemoryResult, QueryFilter};

/// Filtered query execution against a memory store.
///
/// The `Query` trait separates the responsibility of executing
/// filtered queries from the full [`MemoryStore`](crate::store::MemoryStore)
/// interface, enabling specialized query implementations (e.g.,
/// backed by a SQL query planner).
#[async_trait]
pub trait Query: Send + Sync + std::fmt::Debug {
    /// Execute a filtered query and return matching objects.
    async fn query(&self, filter: &QueryFilter) -> MemoryResult<Vec<MemoryObject>>;

    /// Count objects matching a filter without retrieving them.
    async fn count(&self, filter: &QueryFilter) -> MemoryResult<u64>;
}

/// Default no-op implementation of [`Query`].
#[derive(Debug)]
pub struct DefaultQuery;

#[async_trait]
impl Query for DefaultQuery {
    async fn query(&self, _filter: &QueryFilter) -> MemoryResult<Vec<MemoryObject>> {
        Err(memory_core::MemoryError::StorageBackendError("DefaultQuery: not configured".into()))
    }

    async fn count(&self, _filter: &QueryFilter) -> MemoryResult<u64> {
        Err(memory_core::MemoryError::StorageBackendError("DefaultQuery: not configured".into()))
    }
}
