#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! # memory-storage
//!
//! Backend-agnostic storage abstraction for the AI-native OS Memory Platform.
//!
//! ## Traits
//!
//! | Trait | Purpose |
//! |-------|---------|
//! | [`MemoryStore`] | High-level CRUD, batch, query, and management for [`MemoryObject`] records |
//! | [`Transaction`] | Begin, commit, rollback transactional operations |
//! | [`StorageBackend`] | Low-level key-value storage for pluggable database backends |
//! | [`Query`] | Filtered query execution |
//! | [`BatchOperation`] | Efficient batch insert, get, delete |
//!
//! ## Implementations
//!
//! | Type | Description |
//! |------|-------------|
//! | [`InMemoryStore`] | Thread-safe in-memory store backed by `RwLock<HashMap>` |
//! | [`MockStore`] | Configurable mock for testing with call tracking and error injection |

mod backend;
mod store;
mod transaction;

/// Batch operation trait and default implementation.
pub mod batch;
/// Filtered query execution trait and default implementation.
pub mod query;

mod in_memory;
mod mock;

pub use backend::{DefaultStorageBackend, StorageBackend};
pub use batch::{BatchOperation, DefaultBatchOperation};
pub use in_memory::InMemoryStore;
pub use mock::MockStore;
pub use query::{DefaultQuery, Query};
pub use store::{DefaultMemoryStore, MemoryStore};
pub use transaction::{DefaultTransaction, Transaction};
