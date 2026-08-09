//! World Store error type.

use serde::{Deserialize, Serialize};

/// Errors returned by the LIFE World Store.
///
/// Variants follow RFC-0008 §Interfaces. [`WorldStoreError::VersionConflict`]
/// carries the expected and actual version so callers can re-read and re-merge
/// (AI) or reload/force (user).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, thiserror::Error)]
pub enum WorldStoreError {
    /// The writer supplied a stale `expected_version`; the row has moved on.
    #[error("version conflict for {id}: expected {expected}, actual {actual}")]
    VersionConflict {
        /// Entity/relationship/document id.
        id: String,
        /// Version the writer assumed.
        expected: u64,
        /// Version actually stored.
        actual: u64,
    },

    /// Entity does not exist.
    #[error("entity {0} not found")]
    EntityNotFound(String),

    /// Relationship does not exist.
    #[error("relationship {0} not found")]
    RelationshipNotFound(String),

    /// Document does not exist.
    #[error("document {0} not found")]
    DocumentNotFound(String),

    /// Observation does not exist.
    #[error("observation {0} not found")]
    ObservationNotFound(String),

    /// Goal does not exist.
    #[error("goal {0} not found")]
    GoalNotFound(String),

    /// Lesson does not exist.
    #[error("lesson {0} not found")]
    LessonNotFound(String),

    /// An empty/over-length or otherwise invalid entity type was supplied.
    #[error("invalid entity type: {0}")]
    InvalidEntityType(String),

    /// A write context actor is not permitted to perform this operation.
    ///
    /// Part of the RFC-0008 §Interfaces contract but **never raised by the
    /// SQLite store itself**. Authorization is enforced above the store by the
    /// `capability-policy` gate (AI writes) and session roles (user writes) per
    /// ADR-0008 Decision 8 and RFC-0008 §Security; the store records
    /// `created_by`/provenance but performs no permission checks. A future
    /// service/policy layer raises this variant.
    #[error("not authorized: {0}")]
    NotAuthorized(String),

    /// A validation failure at the store boundary (input checks, JSON parse).
    #[error("validation error: {0}")]
    Validation(String),

    /// A general conflict (e.g. duplicate key, referential integrity).
    #[error("conflict: {0}")]
    Conflict(String),

    /// An I/O error (blob storage, backup/restore, migration backup).
    #[error("io error: {0}")]
    Io(String),

    /// A database/storage engine error.
    #[error("storage error: {0}")]
    Storage(String),

    /// The `world_model.json` migration could not be completed.
    #[error("migration error: {0}")]
    Migration(String),
}

impl WorldStoreError {
    /// Whether the error is a transient optimistic-concurrency failure.
    pub fn is_version_conflict(&self) -> bool {
        matches!(self, WorldStoreError::VersionConflict { .. })
    }
}

impl From<rusqlite::Error> for WorldStoreError {
    fn from(err: rusqlite::Error) -> Self {
        WorldStoreError::Storage(err.to_string())
    }
}

impl From<std::io::Error> for WorldStoreError {
    fn from(err: std::io::Error) -> Self {
        WorldStoreError::Io(err.to_string())
    }
}

/// Convenient alias used across the crate.
pub type WorldStoreResult<T> = Result<T, WorldStoreError>;
