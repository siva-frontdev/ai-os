#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! # LIFE World Store
//!
//! The durable substrate for the canonical world model (RFC-0008, ADR-0008).
//! This crate persists the property-graph world model, observations, documents
//! (with FTS5 search), goals, and lessons in SQLite, and exposes:
//!
//! * [`WorldStore`] — a backend-neutral, fallible API for both the AI
//!   cognitive loop and the user (Open Space / companion controls), with
//!   optimistic concurrency, provenance, and full history.
//! * [`SqliteWorldModelStore`] — the SQLite implementation, which also
//!   implements the infallible [`memory_storage::wm_store::WorldModelStore`]
//!   trait consumed by the cognitive loop, plus goal/lesson persistence,
//!   backup/restore, and `world_model.json` migration.
//! * [`WorldStoreService`] — the [`ai_os_core::lifecycle::Service`] adapter.
//!
//! ## Layering
//!
//! This crate is a memory-layer crate (layer 4): it depends on
//! [`ai_os_core`], [`memory_core`], and [`memory_storage`], and nothing above.
//! The only crate that may depend on `rusqlite`.
//!
//! ## Thread model
//!
//! All SQLite access happens on `tokio::task::spawn_blocking`. A single
//! connection behind a `tokio::sync::Mutex` serialises writers (SQLite is a
//! single-writer database) and provides WAL snapshot isolation for reads at
//! personal scale.
//!
//! ## Concurrency
//!
//! Entity, relationship, and document writes are guarded by optimistic
//! concurrency: every request carries `expected_version`, and a stale token
//! yields [`WorldStoreError::VersionConflict`] so the caller can re-read and
//! re-merge (AI) or reload/force (user).

mod api;
mod db;
mod error;
mod events;
mod service;
mod sql;
mod store;
mod types;
mod util;

pub use api::WorldStore;
pub use error::{WorldStoreError, WorldStoreResult};
pub use events::{
    DocumentCreated, EntityArchived, EntityCreated, EntityRestored, EntityUpdated,
    ObservationRecorded, RelationshipCreated,
};
pub use service::WorldStoreService;
pub use store::SqliteWorldModelStore;
pub use types::{
    Actor, BackupManifest, CreateDocument, CreateEntity, CreateRelationship, Document,
    DocumentEntityLink, DocumentId, HistoryEntry, JsonWorldModel, LifecycleFilter, MigrationReport,
    Observation, ObservationId, ObservationProcessState, Operation, RestoreReport, SearchQuery,
    SortBy, StoredGoal, StoredLesson, UpdateDocument, UpdateEntity, UpdateRelationship,
    WorldStoreConfig, WriteContext,
};
