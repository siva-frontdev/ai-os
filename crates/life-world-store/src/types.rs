//! Data types for the LIFE World Store API surface (RFC-0008 §Interfaces).

use std::collections::HashMap;
use std::path::PathBuf;

use memory_core::Timestamp;
use memory_core::wm::{EntityId, EntityLifecycle, RelationshipId, Value};
use serde::{Deserialize, Serialize};

/// Who is performing a write and what justifies it (RFC-0008 §Provenance).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Actor {
    /// The AI cognitive loop / evolution engine.
    Ai,
    /// A specific human user.
    User {
        /// Stable user identifier.
        user_id: String,
    },
    /// A write triggered by an observation.
    Observation,
    /// Platform / runtime service.
    #[default]
    Runtime,
    /// The `world_model.json` migration importer.
    Migration,
}

impl Actor {
    /// The compact writer tag stored in `created_by` columns.
    pub fn tag(&self) -> &'static str {
        match self {
            Actor::Ai => "ai",
            Actor::User { .. } => "user",
            Actor::Observation => "observation",
            Actor::Runtime => "runtime",
            Actor::Migration => "migration",
        }
    }
}

impl std::fmt::Display for Actor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.tag())
    }
}

/// Provenance + actor carried on every World Store write.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WriteContext {
    /// Who performed the write.
    pub actor: Actor,
    /// Observation id / user action id / source URI justifying the write.
    pub provenance: Option<String>,
}

impl WriteContext {
    /// The default AI write context.
    pub fn ai() -> Self {
        Self {
            actor: Actor::Ai,
            provenance: None,
        }
    }

    /// A context carrying the observation that justifies the write.
    pub fn from_observation(observation_id: impl Into<String>) -> Self {
        Self {
            actor: Actor::Observation,
            provenance: Some(observation_id.into()),
        }
    }
}

/// Request to create an entity (RFC-0008 §Interfaces).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateEntity {
    /// Typed catalogue value; free-form types are preserved in
    /// `metadata.original_type`.
    pub entity_type: String,
    /// Display name.
    pub name: String,
    /// Property graph values.
    pub properties: HashMap<String, Value>,
    /// 0..1 importance.
    pub importance: f32,
    /// 0..1 confidence.
    pub confidence: f32,
    /// String metadata (holds `original_type` for normalised writes).
    pub metadata: HashMap<String, String>,
    /// Provenance of the write.
    pub ctx: WriteContext,
}

/// Request to update an entity (optimistic concurrency via `expected_version`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateEntity {
    /// Target entity.
    pub id: EntityId,
    /// Concurrency token; a stale value yields [`WorldStoreError::VersionConflict`].
    pub expected_version: u64,
    /// Replace the name.
    pub name: Option<String>,
    /// Replace the property set.
    pub properties: Option<HashMap<String, Value>>,
    /// Replace the importance.
    pub importance: Option<f32>,
    /// Replace the confidence.
    pub confidence: Option<f32>,
    /// Replace the metadata.
    pub metadata: Option<HashMap<String, String>>,
    /// Provenance of the write.
    pub ctx: WriteContext,
}

/// Request to create a relationship.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRelationship {
    /// Directed edge type (open catalogue, schema §6).
    pub relationship_type: String,
    /// Source endpoint.
    pub source_id: EntityId,
    /// Target endpoint.
    pub target_id: EntityId,
    /// Edge properties.
    pub properties: HashMap<String, Value>,
    /// 0..1 confidence.
    pub confidence: f32,
    /// 0..1 weight.
    pub weight: f32,
    /// Provenance of the write.
    pub ctx: WriteContext,
}

/// Request to update a relationship.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateRelationship {
    /// Target relationship.
    pub id: RelationshipId,
    /// Concurrency token.
    pub expected_version: u64,
    /// Replace the edge type.
    pub relationship_type: Option<String>,
    /// Replace the edge properties.
    pub properties: Option<HashMap<String, Value>>,
    /// Replace the confidence.
    pub confidence: Option<f32>,
    /// Replace the weight.
    pub weight: Option<f32>,
    /// Provenance of the write.
    pub ctx: WriteContext,
}

/// Which lifecycle states a search should include.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LifecycleFilter {
    /// Every lifecycle state.
    All,
    /// `Active` rows only.
    Active,
    /// `Archived` rows only.
    Archived,
}

/// Search ordering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SortBy {
    /// FTS5 relevance (requires a text query).
    Relevance,
    /// Lexicographic by name.
    Name,
    /// Most recently updated first.
    UpdatedAt,
    /// Most recently created first.
    CreatedAt,
}

/// A filtered + paginated search (RFC-0008 §Interfaces).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchQuery {
    /// Free-form text (matched against the FTS5 index when present).
    pub text: Option<String>,
    /// Restrict to a single entity type.
    pub entity_type: Option<String>,
    /// Lifecycle filter.
    pub lifecycle: LifecycleFilter,
    /// Restrict to rows whose metadata contains any of these tag values.
    pub tags: Option<Vec<String>>,
    /// Maximum rows returned.
    pub limit: u32,
    /// Row offset for pagination.
    pub offset: u32,
    /// Ordering.
    pub sort: SortBy,
}

impl Default for SearchQuery {
    fn default() -> Self {
        Self {
            text: None,
            entity_type: None,
            lifecycle: LifecycleFilter::All,
            tags: None,
            limit: 50,
            offset: 0,
            sort: SortBy::UpdatedAt,
        }
    }
}

/// Stable id of a stored document (same UUID as its `document` entity row).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DocumentId(uuid::Uuid);

impl DocumentId {
    /// Generate a fresh id.
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }

    /// Wrap an existing UUID.
    pub fn from_uuid(uuid: uuid::Uuid) -> Self {
        Self(uuid)
    }

    /// The inner UUID.
    pub fn as_uuid(&self) -> &uuid::Uuid {
        &self.0
    }
}

impl Default for DocumentId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for DocumentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Link between a document and an entity (schema §3.3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DocumentEntityLink {
    /// Related entity.
    pub entity_id: EntityId,
    /// Role, e.g. `about`, `author`, `mentioned`.
    pub role: String,
    /// 0..1 confidence.
    pub confidence: f32,
}

/// A stored document (metadata + extracted text + lifecycle).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    /// Same UUID as the corresponding `document` entity.
    pub id: DocumentId,
    /// Display title.
    pub title: String,
    /// MIME type, e.g. `text/markdown`.
    pub mime_type: Option<String>,
    /// Original file size in bytes.
    pub size_bytes: Option<i64>,
    /// Relative path of the checksummed blob under the documents dir.
    pub storage_path: Option<String>,
    /// SHA-256 (hex) of the original file.
    pub checksum: Option<String>,
    /// Original file name at upload.
    pub original_filename: Option<String>,
    /// Origin URI (email id, url, import path).
    pub source_uri: Option<String>,
    /// User tags.
    pub tags: Vec<String>,
    /// Lifecycle (soft delete state).
    pub lifecycle: EntityLifecycle,
    /// Optimistic concurrency token.
    pub version: u64,
    /// Writer tag.
    pub created_by: String,
    /// Provenance of the write.
    pub provenance: Option<String>,
    /// Extracted text.
    pub extracted_text: Option<String>,
    /// Extracted text content type (`plain`, `markdown`, `pdf-text`).
    pub content_type: Option<String>,
    /// Creation timestamp.
    pub created_at: Timestamp,
    /// Last update timestamp.
    pub updated_at: Timestamp,
}

/// Request to create a document (blob + metadata + extracted text).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateDocument {
    /// Display title.
    pub title: String,
    /// MIME type.
    pub mime_type: Option<String>,
    /// Original file name.
    pub original_filename: Option<String>,
    /// Origin URI.
    pub source_uri: Option<String>,
    /// User tags.
    pub tags: Vec<String>,
    /// Original file bytes (stored as a checksummed blob).
    pub content: Vec<u8>,
    /// Extracted text (indexed in `document_fts`).
    pub extracted_text: Option<String>,
    /// Extracted text content type.
    pub content_type: Option<String>,
    /// Extra properties for the document entity row.
    pub entity_properties: HashMap<String, Value>,
    /// Entities extracted from / related to the document.
    pub entity_links: Vec<DocumentEntityLink>,
    /// Provenance of the write.
    pub ctx: WriteContext,
}

/// Request to update a document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateDocument {
    /// Target document.
    pub id: DocumentId,
    /// Concurrency token.
    pub expected_version: u64,
    /// Replace the title.
    pub title: Option<String>,
    /// Replace the tags.
    pub tags: Option<Vec<String>>,
    /// Replace the original content (re-checksummed, blob rewritten).
    pub content: Option<Vec<u8>>,
    /// Replace the extracted text.
    pub extracted_text: Option<String>,
    /// Provenance of the write.
    pub ctx: WriteContext,
}

/// Stable id of a stored observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ObservationId(uuid::Uuid);

impl ObservationId {
    /// Generate a fresh id.
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }

    /// Wrap an existing UUID.
    pub fn from_uuid(uuid: uuid::Uuid) -> Self {
        Self(uuid)
    }

    /// The inner UUID.
    pub fn as_uuid(&self) -> &uuid::Uuid {
        &self.0
    }
}

impl Default for ObservationId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for ObservationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Understanding pipeline state for an observation (schema §2.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObservationProcessState {
    /// Raw, not yet understood.
    Raw = 0,
    /// Understood into a structured world update.
    Understood = 1,
    /// Evolved into the world model.
    Evolved = 2,
}

impl ObservationProcessState {
    /// The stored integer value.
    pub fn as_u8(&self) -> u8 {
        *self as u8
    }
}

/// A durable observation (schema §2.1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    /// Stable id.
    pub id: ObservationId,
    /// Source kind: time | system | desktop | email | telegram | whatsapp |
    /// runtime | user | clock.
    pub kind: String,
    /// Provider/source id (e.g. gmail message id).
    pub source: Option<String>,
    /// Original observation payload.
    pub payload: serde_json::Value,
    /// Natural-language summary (from understanding).
    pub summary: Option<String>,
    /// When it was observed.
    pub observed_at: Timestamp,
    /// 0..1 confidence.
    pub confidence: f32,
    /// Understanding pipeline state.
    pub processed: ObservationProcessState,
    /// `StructuredWorldUpdate.state_changes`.
    pub state_changes: Option<serde_json::Value>,
}

/// The change operation recorded in a history entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Operation {
    /// Row created.
    Create,
    /// Row updated.
    Update,
    /// Row archived (soft deleted).
    Archive,
    /// Row restored.
    Restore,
    /// Row deleted.
    Delete,
}

impl Operation {
    /// The stored text form.
    pub fn as_str(&self) -> &'static str {
        match self {
            Operation::Create => "create",
            Operation::Update => "update",
            Operation::Archive => "archive",
            Operation::Restore => "restore",
            Operation::Delete => "delete",
        }
    }
}

/// One append-only version-history row (schema §1.3).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    /// Entity/relationship id this entry belongs to.
    pub id: String,
    /// The version being recorded.
    pub version: u64,
    /// What happened.
    pub operation: Operation,
    /// Full snapshot at that version (JSON of the canonical row).
    pub snapshot: serde_json::Value,
    /// Writer tag.
    pub changed_by: String,
    /// Provenance of the change.
    pub provenance: Option<String>,
    /// When it happened (unix seconds).
    pub changed_at: Timestamp,
    /// Link to the audit trail, when one was recorded.
    pub audit_id: Option<String>,
}

/// Durable representation of a goal (schema §5.1).
///
/// Mirrors `brain_goals::GoalRecord` operational fields. The brain-layer
/// `GoalStore` trait is implemented over this durable table by an adapter that
/// lives above the memory layer (see RFC-0008 §Goal/Mission Persistence).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StoredGoal {
    /// Stable goal id (UUID string; also the id of the `goal` entity row).
    pub goal_id: String,
    /// Goal type.
    pub goal_type: String,
    /// Human description.
    pub description: String,
    /// Priority tag (`low` | `normal` | `high` | `critical`).
    pub priority: String,
    /// Status tag (snake_case `GoalStatus`).
    pub status: String,
    /// Parent goal id, if any.
    pub parent: Option<String>,
    /// Child goal ids.
    pub children: Vec<String>,
    /// Dependency goal ids.
    pub dependencies: Vec<String>,
    /// Optimistic concurrency token.
    pub version: u64,
    /// Number of retries performed.
    pub retry_count: u64,
    /// Progress percentage 0..100.
    pub progress_pct: f64,
    /// Source of the goal.
    pub source: Option<String>,
    /// Outcome JSON (when completed/failed).
    pub outcome: Option<String>,
    /// Creation timestamp.
    pub created_at: Timestamp,
    /// Last update timestamp.
    pub updated_at: Timestamp,
    /// String metadata.
    pub metadata: HashMap<String, String>,
}

/// Durable representation of a lesson (schema §5.2).
///
/// The brain-layer `LessonStore` is mapped over this table by an adapter above
/// the memory layer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StoredLesson {
    /// Stable lesson id (UUID string).
    pub lesson_id: String,
    /// Lesson category.
    pub category: String,
    /// Lesson content.
    pub content: String,
    /// Source of the lesson.
    pub source: Option<String>,
    /// 0..1 confidence.
    pub confidence: f32,
    /// Creation timestamp.
    pub created_at: Timestamp,
}

/// Backup manifest (RFC-0008 §Backup/Restore).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupManifest {
    /// Unix seconds the backup was created.
    pub created_at: i64,
    /// File name of the snapshot DB inside the backup directory.
    pub db_file: String,
    /// SHA-256 (hex) of the snapshot DB.
    pub db_sha256: String,
    /// SHA-256 (hex) over the documents blob directory.
    pub documents_sha256: String,
}

/// Result of a restore operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreReport {
    /// `PRAGMA integrity_check` result (`ok`).
    pub integrity: String,
    /// Entity count after restore.
    pub entities: u64,
    /// Relationship count after restore.
    pub relationships: u64,
    /// Whether the current documents directory matched the backup checksum.
    pub documents_match: bool,
}

/// Result of the `world_model.json` migration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationReport {
    /// Whether the migration was skipped (no source file, or DB already
    /// migrated).
    pub skipped: bool,
    /// Entities imported.
    pub entities_imported: usize,
    /// Relationships imported.
    pub relationships_imported: usize,
    /// Timestamped backup of the original JSON, when one was made.
    pub source_backup: Option<PathBuf>,
    /// Source file that was migrated.
    pub source_file: Option<PathBuf>,
}

/// Configuration for a [`crate::SqliteWorldModelStore`] (RFC-0008 §Configuration).
#[derive(Debug, Clone)]
pub struct WorldStoreConfig {
    /// SQLite database location. Default:
    /// `~/.local/share/life-os/world/life-world.db`.
    pub db_path: PathBuf,
    /// Document blob root. Default: `~/.local/share/life-os/documents/`.
    pub documents_dir: PathBuf,
    /// Enable WAL journal mode.
    pub wal: bool,
    /// Rotating snapshots kept during backup rotation.
    pub backup_count: u32,
    /// Build/maintain FTS5 indexes.
    pub enable_fts: bool,
    /// Optional `world_model.json` to migrate at first `init`.
    pub json_migration_source: Option<PathBuf>,
}

/// The serialized `world_model.json` shape (schema-compatible with
/// `PersistenceManager::PersistedWorldModel`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonWorldModel {
    /// Entities.
    pub entities: Vec<memory_core::wm::Entity>,
    /// Relationships.
    pub relationships: Vec<memory_core::wm::Relationship>,
    /// Unix seconds the file was written.
    pub saved_at: u64,
}

fn default_data_dir() -> PathBuf {
    if let Some(xdg) = std::env::var_os("XDG_DATA_HOME") {
        return PathBuf::from(xdg);
    }
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home).join(".local").join("share");
    }
    PathBuf::from(".")
}

impl Default for WorldStoreConfig {
    fn default() -> Self {
        let data = default_data_dir();
        Self {
            db_path: data.join("life-os").join("world").join("life-world.db"),
            documents_dir: data.join("life-os").join("documents"),
            wal: true,
            backup_count: 3,
            enable_fts: true,
            json_migration_source: None,
        }
    }
}
