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

mod store;
mod transaction;
mod backend;

/// Filtered query execution trait and default implementation.
pub mod query;
/// Batch operation trait and default implementation.
pub mod batch;

mod in_memory;
mod mock;

pub use store::{MemoryStore, DefaultMemoryStore};
pub use transaction::{Transaction, DefaultTransaction};
pub use backend::{StorageBackend, DefaultStorageBackend};
pub use query::{Query, DefaultQuery};
pub use batch::{BatchOperation, DefaultBatchOperation};
pub use in_memory::InMemoryStore;
pub use mock::MockStore;
