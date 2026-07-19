use async_trait::async_trait;
use memory_core::{MemoryError, MemoryObject, MemoryResult};

/// Transaction support for memory storage operations.
///
/// Provides atomic multi-operation transactions. A transaction is
/// started with [`begin`](Transaction::begin), operations are performed
/// against the transaction ID, and the transaction is either
/// [`commit`](Transaction::commit)ed or [`rollback`](Transaction::rollback)ed.
///
/// All operations within a transaction are isolated from other
/// transactions and from non-transactional operations until committed.
#[async_trait]
pub trait Transaction: Send + Sync + std::fmt::Debug {
    /// Begin a new transaction.
    ///
    /// Returns a unique transaction ID that identifies the transaction
    /// context for subsequent operations.
    async fn begin(&self) -> MemoryResult<uuid::Uuid>;

    /// Commit a transaction, making all changes durable.
    ///
    /// Returns an error if the transaction ID is invalid or the
    /// transaction has already been committed or rolled back.
    async fn commit(&self, tx_id: &uuid::Uuid) -> MemoryResult<()>;

    /// Roll back a transaction, discarding all changes.
    ///
    /// Returns an error if the transaction ID is invalid or the
    /// transaction has already been committed or rolled back.
    async fn rollback(&self, tx_id: &uuid::Uuid) -> MemoryResult<()>;

    /// Insert a memory object within a transaction.
    async fn insert(&self, tx_id: &uuid::Uuid, object: MemoryObject) -> MemoryResult<()>;

    /// Delete a memory object within a transaction.
    async fn delete(&self, tx_id: &uuid::Uuid, id: &memory_core::MemoryId) -> MemoryResult<()>;

    /// Update a memory object within a transaction.
    async fn update(&self, tx_id: &uuid::Uuid, object: MemoryObject) -> MemoryResult<()>;
}

/// Default no-op implementation of [`Transaction`].
#[derive(Debug)]
pub struct DefaultTransaction;

#[async_trait]
impl Transaction for DefaultTransaction {
    async fn begin(&self) -> MemoryResult<uuid::Uuid> {
        Err(MemoryError::TransactionError(
            "DefaultTransaction: not configured".into(),
        ))
    }

    async fn commit(&self, _tx_id: &uuid::Uuid) -> MemoryResult<()> {
        Err(MemoryError::TransactionError(
            "DefaultTransaction: not configured".into(),
        ))
    }

    async fn rollback(&self, _tx_id: &uuid::Uuid) -> MemoryResult<()> {
        Err(MemoryError::TransactionError(
            "DefaultTransaction: not configured".into(),
        ))
    }

    async fn insert(&self, _tx_id: &uuid::Uuid, _object: MemoryObject) -> MemoryResult<()> {
        Err(MemoryError::TransactionError(
            "DefaultTransaction: not configured".into(),
        ))
    }

    async fn delete(&self, _tx_id: &uuid::Uuid, _id: &memory_core::MemoryId) -> MemoryResult<()> {
        Err(MemoryError::TransactionError(
            "DefaultTransaction: not configured".into(),
        ))
    }

    async fn update(&self, _tx_id: &uuid::Uuid, _object: MemoryObject) -> MemoryResult<()> {
        Err(MemoryError::TransactionError(
            "DefaultTransaction: not configured".into(),
        ))
    }
}
