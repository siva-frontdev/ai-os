//! SQLite implementation of the World Store (RFC-0008 §Interfaces).
//!
//! [`SqliteWorldModelStore`] implements both the backend-neutral, fallible
//! [`WorldStore`] API and the infallible
//! [`memory_storage::wm_store::WorldModelStore`] trait used by the cognitive
//! loop. All SQLite access runs on `spawn_blocking`; writes are serialised by a
//! single connection and guarded by optimistic concurrency.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use memory_core::Timestamp;
use memory_core::wm::{Entity, EntityId, EntityLifecycle, Relationship, RelationshipId};
use memory_storage::wm_store::WorldModelStore;
use rusqlite::{Connection, OptionalExtension, params};

use crate::api::WorldStore;
use crate::db::Db;
use crate::error::{WorldStoreError, WorldStoreResult};
use crate::events::{
    DocumentCreated, EntityArchived, EntityCreated, EntityRestored, EntityUpdated,
    ObservationRecorded, RelationshipCreated,
};
use crate::sql::META_MIGRATION_DONE;
use crate::types::{
    BackupManifest, CreateDocument, CreateEntity, CreateRelationship, Document, DocumentId,
    HistoryEntry, LifecycleFilter, MigrationReport, Observation, ObservationId,
    ObservationProcessState, Operation, RestoreReport, SearchQuery, SortBy, StoredGoal,
    StoredLesson, UpdateDocument, UpdateEntity, UpdateRelationship, WorldStoreConfig, WriteContext,
};
use crate::util::*;

/// Canonical column list for the `entities` table.
const ENTITY_COLS: &str = "id, entity_type, name, importance, confidence, lifecycle, version, properties, metadata, \
     created_at, updated_at, created_by, provenance, checksum";

/// Canonical column list for the `relationships` table.
const RELATIONSHIP_COLS: &str = "id, relationship_type, source_id, target_id, confidence, weight, version, properties, \
     created_at, updated_at, created_by, provenance";

/// Canonical column list for the `documents` table.
const DOCUMENT_COLS: &str = "id, title, mime_type, size_bytes, storage_path, checksum, original_filename, source_uri, \
     lifecycle, created_at, updated_at, version, created_by, provenance, tags_json";

/// A row of the `entities` table.
#[derive(Debug)]
struct RawEntity {
    id: String,
    entity_type: String,
    name: String,
    importance: f32,
    confidence: f32,
    lifecycle: String,
    version: u64,
    properties: String,
    metadata: String,
    created_at: i64,
    updated_at: i64,
    created_by: String,
    provenance: Option<String>,
    checksum: Option<String>,
}

/// A row of the `relationships` table.
#[derive(Debug)]
struct RawRelationship {
    id: String,
    relationship_type: String,
    source_id: String,
    target_id: String,
    confidence: f32,
    weight: f32,
    version: u64,
    properties: String,
    created_at: i64,
    updated_at: i64,
    created_by: String,
    provenance: Option<String>,
}

/// A joined row of `documents` + `document_content`.
#[derive(Debug)]
struct RawDocument {
    id: String,
    title: String,
    mime_type: Option<String>,
    size_bytes: Option<i64>,
    storage_path: Option<String>,
    checksum: Option<String>,
    original_filename: Option<String>,
    source_uri: Option<String>,
    lifecycle: String,
    created_at: i64,
    updated_at: i64,
    version: u64,
    created_by: String,
    provenance: Option<String>,
    tags: String,
    extracted_text: Option<String>,
    content_type: Option<String>,
}

/// A row of the `goals` table.
#[derive(Debug)]
struct RawGoal {
    goal_id: String,
    goal_type: String,
    description: String,
    priority: String,
    status: String,
    parent: Option<String>,
    children: String,
    dependencies: String,
    version: u64,
    retry_count: u64,
    progress_pct: f64,
    source: Option<String>,
    outcome: Option<String>,
    created_at: i64,
    updated_at: i64,
    metadata: String,
}

/// The SQLite-backed World Store.
#[derive(Debug)]
pub struct SqliteWorldModelStore {
    db: Db,
    documents_dir: PathBuf,
    backup_count: u32,
    enable_fts: bool,
    bus: Option<Arc<dyn ai_os_core::events::EventBus>>,
}

impl SqliteWorldModelStore {
    /// Create a store without opening the database (opened at [`Self::init`]).
    pub fn new(config: WorldStoreConfig) -> Self {
        Self {
            db: Db::unopened(config.db_path, config.wal),
            documents_dir: config.documents_dir,
            backup_count: config.backup_count,
            enable_fts: config.enable_fts,
            bus: None,
        }
    }

    /// Create a store wired to an [`ai_os_core::events::EventBus`] so writes
    /// publish lifecycle events.
    pub fn with_event_bus(
        config: WorldStoreConfig,
        bus: Arc<dyn ai_os_core::events::EventBus>,
    ) -> Self {
        Self {
            bus: Some(bus),
            ..Self::new(config)
        }
    }

    /// Open the database, apply the schema, and prepare the documents
    /// directory. Idempotent.
    pub async fn init(&self) -> WorldStoreResult<()> {
        self.db.ensure_open().await?;
        std::fs::create_dir_all(&self.documents_dir).map_err(|e| {
            WorldStoreError::Io(format!(
                "create documents dir {}: {e}",
                self.documents_dir.display()
            ))
        })?;
        Ok(())
    }

    /// Checkpoint and close the database connection.
    pub async fn close(&self) -> WorldStoreResult<()> {
        self.db.close().await
    }

    /// Whether the `world_model.json` migration has already completed.
    pub async fn migration_done(&self) -> WorldStoreResult<bool> {
        self.db
            .run(move |conn| {
                let done: Option<String> = conn
                    .query_row(
                        "SELECT value FROM meta WHERE key = ?1",
                        params![META_MIGRATION_DONE],
                        |row| row.get(0),
                    )
                    .optional()?;
                Ok(done.is_some())
            })
            .await
    }

    /// The document blob directory.
    pub fn documents_dir(&self) -> &Path {
        &self.documents_dir
    }

    async fn emit<T: ai_os_core::events::Event>(&self, event: &T) {
        let Some(bus) = &self.bus else {
            return;
        };
        if let Err(e) = bus.publish_event(event).await {
            tracing::warn!(
                event = event.event_type(),
                error = %e,
                "world store event publish failed"
            );
        }
    }
}

// ── Internal row helpers ─────────────────────────────────────────────────────

fn read_raw_entity(row: &rusqlite::Row<'_>) -> rusqlite::Result<RawEntity> {
    Ok(RawEntity {
        id: row.get(0)?,
        entity_type: row.get(1)?,
        name: row.get(2)?,
        importance: row.get(3)?,
        confidence: row.get(4)?,
        lifecycle: row.get(5)?,
        version: row.get::<_, i64>(6)? as u64,
        properties: row.get(7)?,
        metadata: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
        created_by: row.get(11)?,
        provenance: row.get(12)?,
        checksum: row.get(13)?,
    })
}

fn read_raw_relationship(row: &rusqlite::Row<'_>) -> rusqlite::Result<RawRelationship> {
    Ok(RawRelationship {
        id: row.get(0)?,
        relationship_type: row.get(1)?,
        source_id: row.get(2)?,
        target_id: row.get(3)?,
        confidence: row.get(4)?,
        weight: row.get(5)?,
        version: row.get::<_, i64>(6)? as u64,
        properties: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
        created_by: row.get(10)?,
        provenance: row.get(11)?,
    })
}

fn read_raw_document(row: &rusqlite::Row<'_>) -> rusqlite::Result<RawDocument> {
    Ok(RawDocument {
        id: row.get(0)?,
        title: row.get(1)?,
        mime_type: row.get(2)?,
        size_bytes: row.get(3)?,
        storage_path: row.get(4)?,
        checksum: row.get(5)?,
        original_filename: row.get(6)?,
        source_uri: row.get(7)?,
        lifecycle: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
        version: row.get::<_, i64>(11)? as u64,
        created_by: row.get(12)?,
        provenance: row.get(13)?,
        tags: row.get(14)?,
        extracted_text: row.get(15)?,
        content_type: row.get(16)?,
    })
}

fn read_raw_goal(row: &rusqlite::Row<'_>) -> rusqlite::Result<RawGoal> {
    Ok(RawGoal {
        goal_id: row.get(0)?,
        goal_type: row.get(1)?,
        description: row.get(2)?,
        priority: row.get(3)?,
        status: row.get(4)?,
        parent: row.get(5)?,
        children: row.get(6)?,
        dependencies: row.get(7)?,
        version: row.get::<_, i64>(8)? as u64,
        retry_count: row.get::<_, i64>(9)? as u64,
        progress_pct: row.get(10)?,
        source: row.get(11)?,
        outcome: row.get(12)?,
        created_at: row.get(13)?,
        updated_at: row.get(14)?,
        metadata: row.get(15)?,
    })
}

fn raw_entity_to_entity(raw: &RawEntity) -> WorldStoreResult<Entity> {
    Ok(Entity {
        id: str_entity_id(&raw.id)?,
        entity_type: raw.entity_type.clone(),
        name: raw.name.clone(),
        properties: from_json(&raw.properties)?,
        importance: raw.importance,
        confidence: raw.confidence,
        created_at: Timestamp::from_secs(raw.created_at),
        updated_at: Timestamp::from_secs(raw.updated_at),
        version: raw.version,
        lifecycle: str_lifecycle(&raw.lifecycle)?,
        metadata: from_json(&raw.metadata)?,
    })
}

fn raw_relationship_to_relationship(raw: &RawRelationship) -> WorldStoreResult<Relationship> {
    Ok(Relationship {
        id: str_relationship_id(&raw.id)?,
        relationship_type: raw.relationship_type.clone(),
        source_id: str_entity_id(&raw.source_id)?,
        target_id: str_entity_id(&raw.target_id)?,
        properties: from_json(&raw.properties)?,
        confidence: raw.confidence,
        weight: raw.weight,
        created_at: Timestamp::from_secs(raw.created_at),
        updated_at: Timestamp::from_secs(raw.updated_at),
        version: raw.version,
    })
}

fn raw_document_to_document(raw: &RawDocument) -> WorldStoreResult<Document> {
    Ok(Document {
        id: DocumentId::from_uuid(uuid::Uuid::parse_str(&raw.id).map_err(|e| {
            WorldStoreError::Validation(format!("invalid document id {}: {e}", raw.id))
        })?),
        title: raw.title.clone(),
        mime_type: raw.mime_type.clone(),
        size_bytes: raw.size_bytes,
        storage_path: raw.storage_path.clone(),
        checksum: raw.checksum.clone(),
        original_filename: raw.original_filename.clone(),
        source_uri: raw.source_uri.clone(),
        tags: from_json(&raw.tags)?,
        lifecycle: str_lifecycle(&raw.lifecycle)?,
        version: raw.version,
        created_by: raw.created_by.clone(),
        provenance: raw.provenance.clone(),
        extracted_text: raw.extracted_text.clone(),
        content_type: raw.content_type.clone(),
        created_at: Timestamp::from_secs(raw.created_at),
        updated_at: Timestamp::from_secs(raw.updated_at),
    })
}

fn raw_goal_to_goal(raw: &RawGoal) -> WorldStoreResult<StoredGoal> {
    Ok(StoredGoal {
        goal_id: raw.goal_id.clone(),
        goal_type: raw.goal_type.clone(),
        description: raw.description.clone(),
        priority: raw.priority.clone(),
        status: raw.status.clone(),
        parent: raw.parent.clone(),
        children: from_json(&raw.children)?,
        dependencies: from_json(&raw.dependencies)?,
        version: raw.version,
        retry_count: raw.retry_count,
        progress_pct: raw.progress_pct,
        source: raw.source.clone(),
        outcome: raw.outcome.clone(),
        created_at: Timestamp::from_secs(raw.created_at),
        updated_at: Timestamp::from_secs(raw.updated_at),
        metadata: from_json(&raw.metadata)?,
    })
}

fn entity_to_raw(
    entity: &Entity,
    created_by: &str,
    provenance: Option<&str>,
) -> WorldStoreResult<RawEntity> {
    Ok(RawEntity {
        id: entity_id_str(&entity.id),
        entity_type: entity.entity_type.clone(),
        name: entity.name.clone(),
        importance: entity.importance,
        confidence: entity.confidence,
        lifecycle: lifecycle_str(&entity.lifecycle).to_string(),
        version: entity.version,
        properties: to_json(&entity.properties)?,
        metadata: to_json(&entity.metadata)?,
        created_at: entity.created_at.as_secs(),
        updated_at: entity.updated_at.as_secs(),
        created_by: created_by.to_string(),
        provenance: provenance.map(str::to_string),
        checksum: Some(sha256_hex(to_json(entity)?.as_bytes())),
    })
}

fn relationship_to_raw(
    rel: &Relationship,
    created_by: &str,
    provenance: Option<&str>,
) -> WorldStoreResult<RawRelationship> {
    Ok(RawRelationship {
        id: relationship_id_str(&rel.id),
        relationship_type: rel.relationship_type.clone(),
        source_id: entity_id_str(&rel.source_id),
        target_id: entity_id_str(&rel.target_id),
        confidence: rel.confidence,
        weight: rel.weight,
        version: rel.version,
        properties: to_json(&rel.properties)?,
        created_at: rel.created_at.as_secs(),
        updated_at: rel.updated_at.as_secs(),
        created_by: created_by.to_string(),
        provenance: provenance.map(str::to_string),
    })
}

fn value_to_json(value: &serde_json::Value) -> WorldStoreResult<String> {
    serde_json::to_string(value).map_err(|e| WorldStoreError::Validation(e.to_string()))
}

fn get_raw_entity(conn: &Connection, id: &str) -> WorldStoreResult<Option<RawEntity>> {
    conn.query_row(
        &format!("SELECT {ENTITY_COLS} FROM entities WHERE id = ?1"),
        params![id],
        read_raw_entity,
    )
    .optional()
    .map_err(Into::into)
}

fn get_raw_relationship(conn: &Connection, id: &str) -> WorldStoreResult<Option<RawRelationship>> {
    conn.query_row(
        &format!("SELECT {RELATIONSHIP_COLS} FROM relationships WHERE id = ?1"),
        params![id],
        read_raw_relationship,
    )
    .optional()
    .map_err(Into::into)
}

fn get_raw_document(conn: &Connection, id: &str) -> WorldStoreResult<Option<RawDocument>> {
    let sql = format!(
        "SELECT {DOCUMENT_COLS}, c.extracted_text, c.content_type \
         FROM documents d LEFT JOIN document_content c ON c.document_id = d.id \
         WHERE d.id = ?1"
    );
    conn.query_row(&sql, params![id], read_raw_document)
        .optional()
        .map_err(Into::into)
}

fn insert_entity_row(conn: &Connection, raw: &RawEntity) -> WorldStoreResult<()> {
    conn.execute(
        &format!(
            "INSERT INTO entities ({ENTITY_COLS}) \
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)"
        ),
        params![
            raw.id,
            raw.entity_type,
            raw.name,
            raw.importance,
            raw.confidence,
            raw.lifecycle,
            raw.version as i64,
            raw.properties,
            raw.metadata,
            raw.created_at,
            raw.updated_at,
            raw.created_by,
            raw.provenance,
            raw.checksum,
        ],
    )?;
    Ok(())
}

/// Update every mutable column of an entity row (immutable: `id`,
/// `created_at`, `created_by`).
fn update_entity_row(conn: &Connection, raw: &RawEntity) -> WorldStoreResult<()> {
    conn.execute(
        "UPDATE entities SET \
         entity_type = ?2, name = ?3, importance = ?4, confidence = ?5, lifecycle = ?6, \
         version = ?7, properties = ?8, metadata = ?9, updated_at = ?10, provenance = ?11, \
         checksum = ?12 \
         WHERE id = ?1",
        params![
            raw.id,
            raw.entity_type,
            raw.name,
            raw.importance,
            raw.confidence,
            raw.lifecycle,
            raw.version as i64,
            raw.properties,
            raw.metadata,
            raw.updated_at,
            raw.provenance,
            raw.checksum,
        ],
    )?;
    Ok(())
}

fn insert_relationship_row(conn: &Connection, raw: &RawRelationship) -> WorldStoreResult<()> {
    conn.execute(
        &format!(
            "INSERT INTO relationships ({RELATIONSHIP_COLS}) \
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)"
        ),
        params![
            raw.id,
            raw.relationship_type,
            raw.source_id,
            raw.target_id,
            raw.confidence,
            raw.weight,
            raw.version as i64,
            raw.properties,
            raw.created_at,
            raw.updated_at,
            raw.created_by,
            raw.provenance,
        ],
    )?;
    Ok(())
}

fn update_relationship_row(conn: &Connection, raw: &RawRelationship) -> WorldStoreResult<()> {
    conn.execute(
        "UPDATE relationships SET \
         relationship_type = ?2, source_id = ?3, target_id = ?4, confidence = ?5, weight = ?6, \
         version = ?7, properties = ?8, updated_at = ?9, provenance = ?10 \
         WHERE id = ?1",
        params![
            raw.id,
            raw.relationship_type,
            raw.source_id,
            raw.target_id,
            raw.confidence,
            raw.weight,
            raw.version as i64,
            raw.properties,
            raw.updated_at,
            raw.provenance,
        ],
    )?;
    Ok(())
}

// Internal persistence helpers pass many column values as positional args to
// keep the SQL statements readable next to their params! macro; grouping them
// into a struct would add indirection without reducing the call sites.
#[allow(clippy::too_many_arguments)]
fn append_entity_history(
    conn: &Connection,
    id: &str,
    version: u64,
    operation: Operation,
    snapshot: &serde_json::Value,
    changed_by: &str,
    provenance: Option<&str>,
    changed_at: Timestamp,
    audit_id: Option<&str>,
) -> WorldStoreResult<()> {
    conn.execute(
        "INSERT INTO entity_history \
         (entity_id, version, operation, snapshot_json, changed_by, provenance, changed_at, audit_id) \
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
        params![
            id,
            version as i64,
            operation.as_str(),
            value_to_json(snapshot)?,
            changed_by,
            provenance,
            changed_at.as_secs(),
            audit_id,
        ],
    )?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn append_relationship_history(
    conn: &Connection,
    id: &str,
    version: u64,
    operation: Operation,
    snapshot: &serde_json::Value,
    changed_by: &str,
    provenance: Option<&str>,
    changed_at: Timestamp,
) -> WorldStoreResult<()> {
    conn.execute(
        "INSERT INTO relationship_history \
         (relationship_id, version, operation, snapshot_json, changed_by, provenance, changed_at) \
         VALUES (?1,?2,?3,?4,?5,?6,?7)",
        params![
            id,
            version as i64,
            operation.as_str(),
            value_to_json(snapshot)?,
            changed_by,
            provenance,
            changed_at.as_secs(),
        ],
    )?;
    Ok(())
}

fn sync_entity_fts(conn: &Connection, enable_fts: bool, raw: &RawEntity) -> WorldStoreResult<()> {
    if !enable_fts {
        return Ok(());
    }
    conn.execute(
        "DELETE FROM entities_fts WHERE entity_id = ?1",
        params![raw.id],
    )?;
    conn.execute(
        "INSERT INTO entities_fts (entity_id, name, entity_type, properties_text, metadata_text) \
         VALUES (?1,?2,?3,?4,?5)",
        params![
            raw.id,
            raw.name,
            raw.entity_type,
            raw.properties,
            raw.metadata
        ],
    )?;
    Ok(())
}

fn sync_document_fts(
    conn: &Connection,
    enable_fts: bool,
    id: &str,
    title: &str,
    extracted_text: &str,
) -> WorldStoreResult<()> {
    if !enable_fts {
        return Ok(());
    }
    conn.execute(
        "DELETE FROM document_fts WHERE document_id = ?1",
        params![id],
    )?;
    conn.execute(
        "INSERT INTO document_fts (document_id, title, extracted_text) VALUES (?1,?2,?3)",
        params![id, title, extracted_text],
    )?;
    Ok(())
}

fn check_version(existing: u64, expected: u64, id: &str) -> WorldStoreResult<()> {
    if existing != expected {
        return Err(WorldStoreError::VersionConflict {
            id: id.to_string(),
            expected,
            actual: existing,
        });
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn write_entity_snapshot(
    conn: &Connection,
    enable_fts: bool,
    id: &str,
    entity: &Entity,
    operation: Operation,
    changed_by: &str,
    provenance: Option<&str>,
    audit_id: Option<&str>,
) -> WorldStoreResult<()> {
    let raw = entity_to_raw(entity, changed_by, provenance)?;
    let snapshot =
        serde_json::to_value(entity).map_err(|e| WorldStoreError::Validation(e.to_string()))?;
    update_entity_row(conn, &raw)?;
    append_entity_history(
        conn,
        id,
        entity.version,
        operation,
        &snapshot,
        changed_by,
        provenance,
        entity.updated_at,
        audit_id,
    )?;
    sync_entity_fts(conn, enable_fts, &raw)?;
    Ok(())
}

fn write_relationship_snapshot(
    conn: &Connection,
    id: &str,
    rel: &Relationship,
    operation: Operation,
    changed_by: &str,
    provenance: Option<&str>,
) -> WorldStoreResult<()> {
    let raw = relationship_to_raw(rel, changed_by, provenance)?;
    let snapshot =
        serde_json::to_value(rel).map_err(|e| WorldStoreError::Validation(e.to_string()))?;
    update_relationship_row(conn, &raw)?;
    append_relationship_history(
        conn,
        id,
        rel.version,
        operation,
        &snapshot,
        changed_by,
        provenance,
        rel.updated_at,
    )?;
    Ok(())
}

/// Transform a `WorldStore` write context into an immutable provenance tuple.
fn provenance_parts(ctx: &WriteContext) -> (String, Option<String>) {
    (ctx.actor.tag().to_string(), ctx.provenance.clone())
}

// ── WorldStore trait implementation ──────────────────────────────────────────

#[async_trait]
impl WorldStore for SqliteWorldModelStore {
    async fn create_entity(&self, req: CreateEntity) -> WorldStoreResult<Entity> {
        let enable_fts = self.enable_fts;
        let (changed_by, provenance) = provenance_parts(&req.ctx);
        let now = Timestamp::now();
        let mut metadata = req.metadata;
        let entity_type = normalize_entity_type(&req.entity_type, &mut metadata)?;
        let entity = Entity {
            id: EntityId::new(),
            entity_type,
            name: req.name,
            properties: req.properties,
            importance: req.importance.clamp(0.0, 1.0),
            confidence: req.confidence.clamp(0.0, 1.0),
            created_at: now,
            updated_at: now,
            version: 1,
            lifecycle: EntityLifecycle::Active,
            metadata,
        };
        let id_str = entity_id_str(&entity.id);
        let raw = entity_to_raw(&entity, &changed_by, provenance.as_deref())?;
        let snapshot = serde_json::to_value(&entity)
            .map_err(|e| WorldStoreError::Validation(e.to_string()))?;
        self.db
            .run_mut(move |conn| {
                insert_entity_row(conn, &raw)?;
                append_entity_history(
                    conn,
                    &id_str,
                    1,
                    Operation::Create,
                    &snapshot,
                    &changed_by,
                    provenance.as_deref(),
                    now,
                    None,
                )?;
                sync_entity_fts(conn, enable_fts, &raw)?;
                Ok(())
            })
            .await?;
        self.emit(&EntityCreated {
            id: entity.id,
            entity_type: entity.entity_type.clone(),
        })
        .await;
        Ok(entity)
    }

    async fn get_entity(&self, id: &EntityId) -> WorldStoreResult<Option<Entity>> {
        let id_str = entity_id_str(id);
        self.db
            .run(move |conn| {
                let raw = get_raw_entity(conn, &id_str)?;
                raw.map(|r| raw_entity_to_entity(&r)).transpose()
            })
            .await
    }

    async fn update_entity(&self, req: UpdateEntity) -> WorldStoreResult<Entity> {
        let enable_fts = self.enable_fts;
        let id_str = entity_id_str(&req.id);
        let expected = req.expected_version;
        let (changed_by, provenance) = provenance_parts(&req.ctx);
        let entity = self
            .db
            .run_mut(move |conn| {
                let existing = get_raw_entity(conn, &id_str)?
                    .ok_or_else(|| WorldStoreError::EntityNotFound(id_str.clone()))?;
                check_version(existing.version, expected, &id_str)?;
                let mut entity = raw_entity_to_entity(&existing)?;
                if let Some(name) = req.name {
                    entity.name = name;
                }
                if let Some(properties) = req.properties {
                    entity.properties = properties;
                }
                if let Some(importance) = req.importance {
                    entity.importance = importance.clamp(0.0, 1.0);
                }
                if let Some(confidence) = req.confidence {
                    entity.confidence = confidence.clamp(0.0, 1.0);
                }
                if let Some(metadata) = req.metadata {
                    entity.metadata = metadata;
                }
                entity.updated_at = Timestamp::now();
                entity.version += 1;
                let id = entity.id;
                let version = entity.version;
                write_entity_snapshot(
                    conn,
                    enable_fts,
                    &id_str,
                    &entity,
                    Operation::Update,
                    &changed_by,
                    provenance.as_deref(),
                    None,
                )?;
                Ok((id, version, entity))
            })
            .await?;
        self.emit(&EntityUpdated {
            id: entity.0,
            version: entity.1,
        })
        .await;
        Ok(entity.2)
    }

    async fn archive_entity(
        &self,
        id: &EntityId,
        expected_version: u64,
        reason: &str,
        ctx: &WriteContext,
    ) -> WorldStoreResult<()> {
        let enable_fts = self.enable_fts;
        let id_str = entity_id_str(id);
        let (changed_by, provenance) = provenance_parts(ctx);
        self.db
            .run_mut(move |conn| {
                let existing = get_raw_entity(conn, &id_str)?
                    .ok_or_else(|| WorldStoreError::EntityNotFound(id_str.clone()))?;
                check_version(existing.version, expected_version, &id_str)?;
                let mut entity = raw_entity_to_entity(&existing)?;
                entity.lifecycle = EntityLifecycle::Archived;
                entity.updated_at = Timestamp::now();
                entity.version += 1;
                write_entity_snapshot(
                    conn,
                    enable_fts,
                    &id_str,
                    &entity,
                    Operation::Archive,
                    &changed_by,
                    provenance.as_deref(),
                    None,
                )?;
                Ok(())
            })
            .await?;
        self.emit(&EntityArchived {
            id: *id,
            reason: reason.to_string(),
        })
        .await;
        Ok(())
    }

    async fn restore_entity(
        &self,
        id: &EntityId,
        expected_version: u64,
        ctx: &WriteContext,
    ) -> WorldStoreResult<()> {
        let enable_fts = self.enable_fts;
        let id_str = entity_id_str(id);
        let (changed_by, provenance) = provenance_parts(ctx);
        self.db
            .run_mut(move |conn| {
                let existing = get_raw_entity(conn, &id_str)?
                    .ok_or_else(|| WorldStoreError::EntityNotFound(id_str.clone()))?;
                check_version(existing.version, expected_version, &id_str)?;
                let mut entity = raw_entity_to_entity(&existing)?;
                entity.lifecycle = EntityLifecycle::Active;
                entity.updated_at = Timestamp::now();
                entity.version += 1;
                write_entity_snapshot(
                    conn,
                    enable_fts,
                    &id_str,
                    &entity,
                    Operation::Restore,
                    &changed_by,
                    provenance.as_deref(),
                    None,
                )?;
                Ok(())
            })
            .await?;
        self.emit(&EntityRestored { id: *id }).await;
        Ok(())
    }

    async fn delete_entity(
        &self,
        id: &EntityId,
        expected_version: u64,
        hard: bool,
        ctx: &WriteContext,
    ) -> WorldStoreResult<()> {
        let enable_fts = self.enable_fts;
        let documents_dir = self.documents_dir.clone();
        let id_str = entity_id_str(id);
        let (changed_by, provenance) = provenance_parts(ctx);

        if !hard {
            return self
                .archive_entity(id, expected_version, "delete", ctx)
                .await;
        }

        self.db
            .run_mut(move |conn| {
                let existing = get_raw_entity(conn, &id_str)?
                    .ok_or_else(|| WorldStoreError::EntityNotFound(id_str.clone()))?;
                check_version(existing.version, expected_version, &id_str)?;
                let entity = raw_entity_to_entity(&existing)?;
                let snapshot = serde_json::to_value(&entity)
                    .map_err(|e| WorldStoreError::Validation(e.to_string()))?;

                let tx = conn.transaction()?;
                tx.execute(
                    "DELETE FROM document_entities WHERE entity_id = ?1",
                    params![id_str],
                )?;
                tx.execute(
                    "DELETE FROM entity_observations WHERE entity_id = ?1",
                    params![id_str],
                )?;

                // Document entities additionally carry `documents` /
                // `document_content` rows and a managed blob. Hard delete
                // purges those too; the blob is retained until this point
                // (ADR-0008 Decision 4).
                let document_storage_path: Option<String> = tx
                    .prepare("SELECT storage_path FROM documents WHERE id = ?1")?
                    .query_row(params![id_str], |row| row.get(0))
                    .optional()?
                    .flatten();
                if document_storage_path.is_some() {
                    tx.execute(
                        "DELETE FROM document_content WHERE document_id = ?1",
                        params![id_str],
                    )?;
                    tx.execute("DELETE FROM documents WHERE id = ?1", params![id_str])?;
                }

                let rel_ids: Vec<String> = {
                    let mut stmt = tx.prepare(
                        "SELECT id FROM relationships WHERE source_id = ?1 OR target_id = ?1",
                    )?;
                    let rows = stmt.query_map(params![id_str], |row| row.get::<_, String>(0))?;
                    let mut out = Vec::new();
                    for r in rows {
                        out.push(r?);
                    }
                    out
                };
                for rid in &rel_ids {
                    tx.execute(
                        "DELETE FROM relationship_history WHERE relationship_id = ?1",
                        params![rid],
                    )?;
                }
                tx.execute(
                    "DELETE FROM relationships WHERE source_id = ?1 OR target_id = ?1",
                    params![id_str],
                )?;

                // Drop the previous history, then record the final Delete
                // tombstone (the entity row still exists at this point).
                tx.execute(
                    "DELETE FROM entity_history WHERE entity_id = ?1",
                    params![id_str],
                )?;
                append_entity_history(
                    &tx,
                    &id_str,
                    existing.version,
                    Operation::Delete,
                    &snapshot,
                    &changed_by,
                    provenance.as_deref(),
                    entity.updated_at,
                    None,
                )?;
                if enable_fts {
                    tx.execute(
                        "DELETE FROM entities_fts WHERE entity_id = ?1",
                        params![id_str],
                    )?;
                }
                // Remove the managed blob before commit so a failure aborts the
                // whole delete (entity and blob stay consistent). The stored
                // path is generated internally as `<uuid>.bin`; reject anything
                // that could escape the documents directory (RFC-0008 §Security
                // path canonicalization).
                if let Some(storage_path) = document_storage_path {
                    if storage_path.contains('/')
                        || storage_path.contains('\\')
                        || storage_path.contains("..")
                    {
                        return Err(WorldStoreError::Validation(format!(
                            "document storage path {storage_path:?} escapes the documents directory"
                        )));
                    }
                    let blob_path = documents_dir.join(&storage_path);
                    match std::fs::remove_file(&blob_path) {
                        Ok(()) => {}
                        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                        Err(e) => {
                            return Err(WorldStoreError::Io(format!(
                                "remove document blob {:?}: {e}",
                                blob_path
                            )));
                        }
                    }
                }
                tx.execute("DELETE FROM entities WHERE id = ?1", params![id_str])?;
                tx.commit()?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    async fn create_relationship(&self, req: CreateRelationship) -> WorldStoreResult<Relationship> {
        let (changed_by, provenance) = provenance_parts(&req.ctx);
        let now = Timestamp::now();
        let rel = Relationship {
            id: RelationshipId::new(),
            relationship_type: req.relationship_type,
            source_id: req.source_id,
            target_id: req.target_id,
            properties: req.properties,
            confidence: req.confidence.clamp(0.0, 1.0),
            weight: req.weight.clamp(0.0, 1.0),
            created_at: now,
            updated_at: now,
            version: 1,
        };
        let id_str = relationship_id_str(&rel.id);
        let raw = relationship_to_raw(&rel, &changed_by, provenance.as_deref())?;
        let snapshot =
            serde_json::to_value(&rel).map_err(|e| WorldStoreError::Validation(e.to_string()))?;
        self.db
            .run_mut(move |conn| {
                insert_relationship_row(conn, &raw)?;
                append_relationship_history(
                    conn,
                    &id_str,
                    1,
                    Operation::Create,
                    &snapshot,
                    &changed_by,
                    provenance.as_deref(),
                    now,
                )?;
                Ok(())
            })
            .await?;
        self.emit(&RelationshipCreated {
            id: rel.id,
            source: rel.source_id,
            target: rel.target_id,
        })
        .await;
        Ok(rel)
    }

    async fn get_relationship(
        &self,
        id: &RelationshipId,
    ) -> WorldStoreResult<Option<Relationship>> {
        let id_str = relationship_id_str(id);
        self.db
            .run(move |conn| {
                let raw = get_raw_relationship(conn, &id_str)?;
                raw.map(|r| raw_relationship_to_relationship(&r))
                    .transpose()
            })
            .await
    }

    async fn update_relationship(&self, req: UpdateRelationship) -> WorldStoreResult<Relationship> {
        let id_str = relationship_id_str(&req.id);
        let expected = req.expected_version;
        let (changed_by, provenance) = provenance_parts(&req.ctx);
        let rel = self
            .db
            .run_mut(move |conn| {
                let existing = get_raw_relationship(conn, &id_str)?
                    .ok_or_else(|| WorldStoreError::RelationshipNotFound(id_str.clone()))?;
                check_version(existing.version, expected, &id_str)?;
                let mut rel = raw_relationship_to_relationship(&existing)?;
                if let Some(relationship_type) = req.relationship_type {
                    rel.relationship_type = relationship_type;
                }
                if let Some(properties) = req.properties {
                    rel.properties = properties;
                }
                if let Some(confidence) = req.confidence {
                    rel.confidence = confidence.clamp(0.0, 1.0);
                }
                if let Some(weight) = req.weight {
                    rel.weight = weight.clamp(0.0, 1.0);
                }
                rel.updated_at = Timestamp::now();
                rel.version += 1;
                write_relationship_snapshot(
                    conn,
                    &id_str,
                    &rel,
                    Operation::Update,
                    &changed_by,
                    provenance.as_deref(),
                )?;
                Ok(rel)
            })
            .await?;
        Ok(rel)
    }

    async fn delete_relationship(
        &self,
        id: &RelationshipId,
        expected_version: u64,
        ctx: &WriteContext,
    ) -> WorldStoreResult<()> {
        let id_str = relationship_id_str(id);
        let (changed_by, provenance) = provenance_parts(ctx);
        self.db
            .run_mut(move |conn| {
                let existing = get_raw_relationship(conn, &id_str)?
                    .ok_or_else(|| WorldStoreError::RelationshipNotFound(id_str.clone()))?;
                check_version(existing.version, expected_version, &id_str)?;
                let rel = raw_relationship_to_relationship(&existing)?;
                let snapshot = serde_json::to_value(&rel)
                    .map_err(|e| WorldStoreError::Validation(e.to_string()))?;
                // Tombstone at `version + 1` so it does not collide with the
                // existing history entry for the final version; the row is
                // deleted inside the same transaction for atomicity.
                let tx = conn.transaction()?;
                append_relationship_history(
                    &tx,
                    &id_str,
                    existing.version + 1,
                    Operation::Delete,
                    &snapshot,
                    &changed_by,
                    provenance.as_deref(),
                    rel.updated_at,
                )?;
                tx.execute("DELETE FROM relationships WHERE id = ?1", params![id_str])?;
                tx.commit()?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    async fn search_entities(&self, query: &SearchQuery) -> WorldStoreResult<Vec<Entity>> {
        let enable_fts = self.enable_fts;
        let cloned = query.clone();
        self.db
            .run(move |conn| search_entities_sql(conn, &cloned, enable_fts))
            .await
    }

    async fn search_documents(&self, query: &SearchQuery) -> WorldStoreResult<Vec<Document>> {
        let enable_fts = self.enable_fts;
        let cloned = query.clone();
        self.db
            .run(move |conn| search_documents_sql(conn, &cloned, enable_fts))
            .await
    }

    async fn record_observation(&self, obs: Observation) -> WorldStoreResult<ObservationId> {
        let id = obs.id;
        let kind = obs.kind.clone();
        let id_str = id.to_string();
        self.db
            .run_mut(move |conn| {
                conn.execute(
                    "INSERT INTO observations \
                     (id, kind, source, payload_json, summary, observed_at, confidence, processed, state_changes_json) \
                     VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
                    params![
                        id_str,
                        obs.kind,
                        obs.source,
                        value_to_json(&obs.payload)?,
                        obs.summary,
                        obs.observed_at.as_secs(),
                        obs.confidence,
                        obs.processed.as_u8(),
                        obs.state_changes
                            .as_ref()
                            .map(value_to_json)
                            .transpose()?,
                    ],
                )?;
                Ok(())
            })
            .await?;
        self.emit(&ObservationRecorded { id, kind }).await;
        Ok(id)
    }

    async fn get_observation(&self, id: &ObservationId) -> WorldStoreResult<Option<Observation>> {
        let id_str = id.to_string();
        self.db
            .run(move |conn| {
                let raw: Option<(
                    String,
                    String,
                    Option<String>,
                    String,
                    Option<String>,
                    i64,
                    f32,
                    i64,
                    Option<String>,
                )> = conn
                    .query_row(
                        "SELECT id, kind, source, payload_json, summary, observed_at, confidence, \
                         processed, state_changes_json FROM observations WHERE id = ?1",
                        params![id_str],
                        |row| {
                            Ok((
                                row.get(0)?,
                                row.get(1)?,
                                row.get(2)?,
                                row.get(3)?,
                                row.get(4)?,
                                row.get(5)?,
                                row.get(6)?,
                                row.get(7)?,
                                row.get(8)?,
                            ))
                        },
                    )
                    .optional()?;
                let Some((
                    id,
                    kind,
                    source,
                    payload_json,
                    summary,
                    observed_at,
                    confidence,
                    processed,
                    state_changes_json,
                )) = raw
                else {
                    return Ok(None);
                };
                let payload: serde_json::Value = from_json(&payload_json)?;
                let state_changes: Option<serde_json::Value> =
                    state_changes_json.as_deref().map(from_json).transpose()?;
                Ok(Some(Observation {
                    id: ObservationId::from_uuid(uuid::Uuid::parse_str(&id).map_err(|e| {
                        WorldStoreError::Validation(format!("invalid observation id: {e}"))
                    })?),
                    kind,
                    source,
                    payload,
                    summary,
                    observed_at: Timestamp::from_secs(observed_at),
                    confidence,
                    processed: match processed {
                        0 => ObservationProcessState::Raw,
                        1 => ObservationProcessState::Understood,
                        _ => ObservationProcessState::Evolved,
                    },
                    state_changes,
                }))
            })
            .await
    }

    async fn get_history(&self, id: &EntityId) -> WorldStoreResult<Vec<HistoryEntry>> {
        let id_str = entity_id_str(id);
        self.db
            .run(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT entity_id, version, operation, snapshot_json, changed_by, provenance, \
                     changed_at, audit_id \
                     FROM entity_history WHERE entity_id = ?1 ORDER BY version ASC",
                )?;
                let rows = stmt.query_map(params![id_str], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, Option<String>>(5)?,
                        row.get::<_, i64>(6)?,
                        row.get::<_, Option<String>>(7)?,
                    ))
                })?;
                let mut out = Vec::new();
                for row in rows {
                    let (
                        id,
                        version,
                        operation,
                        snapshot_json,
                        changed_by,
                        provenance,
                        changed_at,
                        audit_id,
                    ) = row?;
                    out.push(HistoryEntry {
                        id,
                        version: version as u64,
                        operation: match operation.as_str() {
                            "create" => Operation::Create,
                            "update" => Operation::Update,
                            "archive" => Operation::Archive,
                            "restore" => Operation::Restore,
                            _ => Operation::Delete,
                        },
                        snapshot: from_json(&snapshot_json)?,
                        changed_by,
                        provenance,
                        changed_at: Timestamp::from_secs(changed_at),
                        audit_id,
                    });
                }
                Ok(out)
            })
            .await
    }

    async fn create_document(&self, req: CreateDocument) -> WorldStoreResult<Document> {
        let enable_fts = self.enable_fts;
        let documents_dir = self.documents_dir.clone();
        let (changed_by, provenance) = provenance_parts(&req.ctx);
        let now = Timestamp::now();
        let id = DocumentId::new();
        let id_str = id.to_string();
        let checksum = sha256_hex(&req.content);
        let size_bytes = req.content.len() as i64;
        let extracted_text = req.extracted_text.unwrap_or_default();
        let doc_extracted_text = if extracted_text.is_empty() {
            None
        } else {
            Some(extracted_text.clone())
        };

        std::fs::create_dir_all(&documents_dir)
            .map_err(|e| WorldStoreError::Io(format!("create documents dir: {e}")))?;
        let blob_name = format!("{id_str}.bin");
        let blob_path = documents_dir.join(&blob_name);
        std::fs::write(&blob_path, &req.content)
            .map_err(|e| WorldStoreError::Io(format!("write document blob {blob_path:?}: {e}")))?;

        let tags = to_json(&req.tags)?;
        let content_type = req.content_type.clone();

        let entity = Entity {
            id: EntityId::from_uuid(*id.as_uuid()),
            entity_type: "document".to_string(),
            name: req.title.clone(),
            properties: req.entity_properties,
            importance: 0.5,
            confidence: 0.5,
            created_at: now,
            updated_at: now,
            version: 1,
            lifecycle: EntityLifecycle::Active,
            metadata: HashMap::new(),
        };
        let raw_entity = entity_to_raw(&entity, &changed_by, provenance.as_deref())?;

        let doc = Document {
            id,
            title: req.title.clone(),
            mime_type: req.mime_type,
            size_bytes: Some(size_bytes),
            storage_path: Some(blob_name),
            checksum: Some(checksum),
            original_filename: req.original_filename,
            source_uri: req.source_uri,
            tags: req.tags,
            lifecycle: EntityLifecycle::Active,
            version: 1,
            created_by: changed_by.clone(),
            provenance: provenance.clone(),
            extracted_text: doc_extracted_text,
            content_type: content_type.clone(),
            created_at: now,
            updated_at: now,
        };

        let entity_links = req.entity_links;
        let doc = self
            .db
            .run_mut(move |conn| {
                let tx = conn.transaction()?;
                insert_entity_row(&tx, &raw_entity)?;
                tx.execute(
                    "INSERT INTO documents \
                     (id, title, mime_type, size_bytes, storage_path, checksum, original_filename, \
                      source_uri, lifecycle, created_at, updated_at, version, created_by, provenance, tags_json) \
                     VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",
                    params![
                        id_str,
                        doc.title,
                        doc.mime_type,
                        doc.size_bytes,
                        doc.storage_path,
                        doc.checksum,
                        doc.original_filename,
                        doc.source_uri,
                        lifecycle_str(&doc.lifecycle),
                        doc.created_at.as_secs(),
                        doc.updated_at.as_secs(),
                        doc.version as i64,
                        doc.created_by,
                        doc.provenance,
                        tags,
                    ],
                )?;
                if !extracted_text.is_empty() {
                    tx.execute(
                        "INSERT INTO document_content \
                         (document_id, extracted_text, content_type, content_hash) \
                         VALUES (?1,?2,?3,?4)",
                        params![id_str, extracted_text, content_type, doc.checksum],
                    )?;
                }
                for link in &entity_links {
                    tx.execute(
                        "INSERT INTO document_entities (document_id, entity_id, role, confidence) \
                         VALUES (?1,?2,?3,?4)",
                        params![
                            id_str,
                            entity_id_str(&link.entity_id),
                            link.role,
                            link.confidence,
                        ],
                    )?;
                }
                sync_document_fts(&tx, enable_fts, &id_str, &doc.title, &extracted_text)?;
                tx.commit()?;
                Ok(doc)
            })
            .await?;
        self.emit(&DocumentCreated { id }).await;
        Ok(doc)
    }

    async fn get_document(&self, id: &DocumentId) -> WorldStoreResult<Option<Document>> {
        let id_str = id.to_string();
        self.db
            .run(move |conn| {
                let raw = get_raw_document(conn, &id_str)?;
                raw.map(|r| raw_document_to_document(&r)).transpose()
            })
            .await
    }

    async fn update_document(&self, req: UpdateDocument) -> WorldStoreResult<Document> {
        let enable_fts = self.enable_fts;
        let documents_dir = self.documents_dir.clone();

        let UpdateDocument {
            id,
            expected_version,
            title,
            tags,
            content,
            extracted_text,
            ctx,
        } = req;

        let id_str = id.to_string();
        let (_changed_by, provenance) = provenance_parts(&ctx);
        let (new_checksum, new_size) = match &content {
            Some(bytes) => (Some(sha256_hex(bytes)), Some(bytes.len() as i64)),
            None => (None, None),
        };

        let doc = self
            .db
            .run_mut(move |conn| {
                let existing = get_raw_document(conn, &id_str)?
                    .ok_or_else(|| WorldStoreError::DocumentNotFound(id_str.clone()))?;
                check_version(existing.version, expected_version, &id_str)?;
                let mut doc = raw_document_to_document(&existing)?;
                if let Some(content_bytes) = content {
                    std::fs::write(documents_dir.join(format!("{id_str}.bin")), &content_bytes)
                        .map_err(|e| WorldStoreError::Io(format!("rewrite document blob: {e}")))?;
                }
                if let Some(title) = title {
                    doc.title = title;
                }
                if let Some(tags) = tags {
                    doc.tags = tags;
                }
                if let Some(checksum) = &new_checksum {
                    doc.checksum = Some(checksum.clone());
                }
                if let Some(size) = new_size {
                    doc.size_bytes = Some(size);
                }
                if let Some(extracted_text) = extracted_text {
                    let hash = doc.checksum.clone();
                    let content_type = doc.content_type.clone();
                    conn.execute(
                        "INSERT INTO document_content \
                         (document_id, extracted_text, content_type, content_hash) \
                         VALUES (?1,?2,?3,?4) \
                         ON CONFLICT(document_id) DO UPDATE SET \
                         extracted_text = excluded.extracted_text, \
                         content_type = excluded.content_type, \
                         content_hash = excluded.content_hash",
                        params![id_str, extracted_text, content_type, hash],
                    )?;
                    doc.extracted_text = Some(extracted_text);
                }
                let title = doc.title.clone();
                let text = doc.extracted_text.clone().unwrap_or_default();
                sync_document_fts(conn, enable_fts, &id_str, &title, &text)?;
                doc.updated_at = Timestamp::now();
                doc.version += 1;
                conn.execute(
                    "UPDATE documents SET \
                     title = ?2, size_bytes = ?3, checksum = ?4, updated_at = ?5, version = ?6, \
                     tags_json = ?7, provenance = ?8 \
                     WHERE id = ?1",
                    params![
                        id_str,
                        doc.title,
                        doc.size_bytes,
                        doc.checksum,
                        doc.updated_at.as_secs(),
                        doc.version as i64,
                        to_json(&doc.tags)?,
                        provenance,
                    ],
                )?;
                Ok(doc)
            })
            .await?;
        Ok(doc)
    }

    async fn archive_document(
        &self,
        id: &DocumentId,
        expected_version: u64,
        ctx: &WriteContext,
    ) -> WorldStoreResult<()> {
        let id_str = id.to_string();
        let (_changed_by, provenance) = provenance_parts(ctx);
        self.db
            .run_mut(move |conn| {
                let existing = get_raw_document(conn, &id_str)?
                    .ok_or_else(|| WorldStoreError::DocumentNotFound(id_str.clone()))?;
                check_version(existing.version, expected_version, &id_str)?;
                let version = existing.version + 1;
                conn.execute(
                    "UPDATE documents SET lifecycle = ?2, version = ?3, updated_at = ?4, \
                     provenance = ?5 \
                     WHERE id = ?1",
                    params![
                        id_str,
                        "Archived",
                        version as i64,
                        Timestamp::now().as_secs(),
                        provenance,
                    ],
                )?;
                Ok(())
            })
            .await?;
        Ok(())
    }
}

// ── Search SQL builders ──────────────────────────────────────────────────────

fn lifecycle_sql(filter: LifecycleFilter) -> Option<&'static str> {
    match filter {
        LifecycleFilter::All => None,
        LifecycleFilter::Active => Some("Active"),
        LifecycleFilter::Archived => Some("Archived"),
    }
}

fn search_entities_sql(
    conn: &Connection,
    query: &SearchQuery,
    enable_fts: bool,
) -> WorldStoreResult<Vec<Entity>> {
    let mut sql = String::from(
        "SELECT e.id, e.entity_type, e.name, e.importance, e.confidence, e.lifecycle, \
         e.version, e.properties, e.metadata, e.created_at, e.updated_at, e.created_by, \
         e.provenance, e.checksum FROM entities e",
    );
    let mut conditions: Vec<String> = Vec::new();
    let mut params: Vec<rusqlite::types::Value> = Vec::new();

    if let Some(lc) = lifecycle_sql(query.lifecycle) {
        conditions.push("e.lifecycle = ?".to_string());
        params.push(rusqlite::types::Value::Text(lc.to_string()));
    }
    if let Some(entity_type) = &query.entity_type {
        conditions.push("e.entity_type = ?".to_string());
        params.push(rusqlite::types::Value::Text(entity_type.clone()));
    }
    if let Some(tags) = &query.tags
        && !tags.is_empty()
    {
        let placeholders = std::iter::repeat_n("?", tags.len())
            .collect::<Vec<_>>()
            .join(",");
        conditions.push(format!(
            "EXISTS (SELECT 1 FROM json_each(e.metadata) je WHERE je.value IN ({placeholders}))"
        ));
        for tag in tags {
            params.push(rusqlite::types::Value::Text(tag.clone()));
        }
    }

    let use_fts = enable_fts && query.text.as_deref().is_some_and(|t| !t.trim().is_empty());
    let text = query.text.clone().unwrap_or_default();
    if use_fts {
        sql.push_str(" JOIN entities_fts f ON f.entity_id = e.id");
        conditions.push("entities_fts MATCH ?".to_string());
        params.push(rusqlite::types::Value::Text(fts_query(&text)));
    } else if !text.trim().is_empty() {
        conditions.push("e.name LIKE ?".to_string());
        params.push(rusqlite::types::Value::Text(format!("%{}%", text)));
    }

    if !conditions.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&conditions.join(" AND "));
    }

    let order = match query.sort {
        SortBy::Relevance if use_fts => " ORDER BY bm25(entities_fts)".to_string(),
        SortBy::Relevance => " ORDER BY e.name COLLATE NOCASE ASC".to_string(),
        SortBy::Name => " ORDER BY e.name COLLATE NOCASE ASC".to_string(),
        SortBy::UpdatedAt => " ORDER BY e.updated_at DESC".to_string(),
        SortBy::CreatedAt => " ORDER BY e.created_at DESC".to_string(),
    };
    sql.push_str(&order);
    sql.push_str(" LIMIT ? OFFSET ?");
    params.push(rusqlite::types::Value::Integer(query.limit as i64));
    params.push(rusqlite::types::Value::Integer(query.offset as i64));

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(params.iter()), read_raw_entity)?;
    let mut out = Vec::new();
    for row in rows {
        let raw = row?;
        out.push(raw_entity_to_entity(&raw)?);
    }
    Ok(out)
}

fn search_documents_sql(
    conn: &Connection,
    query: &SearchQuery,
    enable_fts: bool,
) -> WorldStoreResult<Vec<Document>> {
    let mut sql = String::from(
        "SELECT d.id, d.title, d.mime_type, d.size_bytes, d.storage_path, d.checksum, \
         d.original_filename, d.source_uri, d.lifecycle, d.created_at, d.updated_at, d.version, \
         d.created_by, d.provenance, d.tags_json, c.extracted_text, c.content_type \
         FROM documents d LEFT JOIN document_content c ON c.document_id = d.id",
    );
    let mut conditions: Vec<String> = Vec::new();
    let mut params: Vec<rusqlite::types::Value> = Vec::new();

    if let Some(lc) = lifecycle_sql(query.lifecycle) {
        conditions.push("d.lifecycle = ?".to_string());
        params.push(rusqlite::types::Value::Text(lc.to_string()));
    }
    if let Some(tags) = &query.tags
        && !tags.is_empty()
    {
        let placeholders = std::iter::repeat_n("?", tags.len())
            .collect::<Vec<_>>()
            .join(",");
        conditions.push(format!(
            "EXISTS (SELECT 1 FROM json_each(d.tags_json) je WHERE je.value IN ({placeholders}))"
        ));
        for tag in tags {
            params.push(rusqlite::types::Value::Text(tag.clone()));
        }
    }

    let use_fts = enable_fts && query.text.as_deref().is_some_and(|t| !t.trim().is_empty());
    let text = query.text.clone().unwrap_or_default();
    if use_fts {
        sql.push_str(" JOIN document_fts f ON f.document_id = d.id");
        conditions.push("document_fts MATCH ?".to_string());
        params.push(rusqlite::types::Value::Text(fts_query(&text)));
    } else if !text.trim().is_empty() {
        conditions.push("d.title LIKE ?".to_string());
        params.push(rusqlite::types::Value::Text(format!("%{}%", text)));
    }

    if !conditions.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&conditions.join(" AND "));
    }

    let order = match query.sort {
        SortBy::Relevance if use_fts => " ORDER BY bm25(document_fts)".to_string(),
        SortBy::Relevance => " ORDER BY d.title COLLATE NOCASE ASC".to_string(),
        SortBy::Name => " ORDER BY d.title COLLATE NOCASE ASC".to_string(),
        SortBy::UpdatedAt => " ORDER BY d.updated_at DESC".to_string(),
        SortBy::CreatedAt => " ORDER BY d.created_at DESC".to_string(),
    };
    sql.push_str(&order);
    sql.push_str(" LIMIT ? OFFSET ?");
    params.push(rusqlite::types::Value::Integer(query.limit as i64));
    params.push(rusqlite::types::Value::Integer(query.offset as i64));

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(params.iter()), read_raw_document)?;
    let mut out = Vec::new();
    for row in rows {
        let raw = row?;
        out.push(raw_document_to_document(&raw)?);
    }
    Ok(out)
}

// ── WorldModelStore trait implementation (cognitive path) ────────────────────

#[async_trait]
impl WorldModelStore for SqliteWorldModelStore {
    async fn insert_entity(&self, entity: Entity) -> EntityId {
        let enable_fts = self.enable_fts;
        let result_id = entity.id;
        let id_str = entity_id_str(&entity.id);
        let raw = match entity_to_raw(&entity, "ai", None) {
            Ok(raw) => raw,
            Err(e) => {
                tracing::warn!(error = %e, "world store: failed to serialise entity");
                return result_id;
            }
        };
        let snapshot = serde_json::to_value(&entity).unwrap_or_default();
        let changed_by = "ai".to_string();
        let version = entity.version;
        let updated_at = entity.updated_at;
        let warn_id = id_str.clone();
        if let Err(e) = self
            .db
            .run_mut(move |conn| {
                let existing = get_raw_entity(conn, &id_str)?;
                if existing.is_some() {
                    update_entity_row(conn, &raw)?;
                } else {
                    insert_entity_row(conn, &raw)?;
                }
                append_entity_history(
                    conn,
                    &id_str,
                    version,
                    Operation::Create,
                    &snapshot,
                    &changed_by,
                    None,
                    updated_at,
                    None,
                )?;
                sync_entity_fts(conn, enable_fts, &raw)?;
                Ok(())
            })
            .await
        {
            tracing::warn!(error = %e, id = %warn_id, "world store: insert_entity failed");
        }
        result_id
    }

    async fn get_entity(&self, id: &EntityId) -> Option<Entity> {
        let id_str = entity_id_str(id);
        self.db
            .run(move |conn| {
                let raw = get_raw_entity(conn, &id_str)?;
                raw.map(|r| raw_entity_to_entity(&r)).transpose()
            })
            .await
            .ok()
            .flatten()
    }

    async fn update_entity(&self, entity: &Entity) {
        let enable_fts = self.enable_fts;
        let id_str = entity_id_str(&entity.id);
        let raw = match entity_to_raw(entity, "ai", None) {
            Ok(raw) => raw,
            Err(e) => {
                tracing::warn!(error = %e, "world store: failed to serialise entity update");
                return;
            }
        };
        let snapshot = serde_json::to_value(entity).unwrap_or_default();
        let changed_by = "ai".to_string();
        let version = entity.version;
        let updated_at = entity.updated_at;
        let warn_id = id_str.clone();
        if let Err(e) = self
            .db
            .run_mut(move |conn| {
                update_entity_row(conn, &raw)?;
                append_entity_history(
                    conn,
                    &id_str,
                    version,
                    Operation::Update,
                    &snapshot,
                    &changed_by,
                    None,
                    updated_at,
                    None,
                )?;
                sync_entity_fts(conn, enable_fts, &raw)?;
                Ok(())
            })
            .await
        {
            tracing::warn!(error = %e, id = %warn_id, "world store: update_entity failed");
        }
    }

    async fn delete_entity(&self, id: &EntityId) {
        let enable_fts = self.enable_fts;
        let id_str = entity_id_str(id);
        let _ = self
            .db
            .run_mut(move |conn| {
                let existing = match get_raw_entity(conn, &id_str)? {
                    Some(raw) => raw,
                    None => return Ok(()),
                };
                let mut entity = match raw_entity_to_entity(&existing) {
                    Ok(e) => e,
                    Err(_) => return Ok(()),
                };
                entity.lifecycle = EntityLifecycle::Archived;
                entity.updated_at = Timestamp::now();
                entity.version += 1;
                let snapshot = serde_json::to_value(&entity).unwrap_or_default();
                let raw = match entity_to_raw(&entity, "ai", None) {
                    Ok(raw) => raw,
                    Err(_) => return Ok(()),
                };
                update_entity_row(conn, &raw)?;
                append_entity_history(
                    conn,
                    &id_str,
                    entity.version,
                    Operation::Archive,
                    &snapshot,
                    "ai",
                    None,
                    entity.updated_at,
                    None,
                )?;
                sync_entity_fts(conn, enable_fts, &raw)?;
                Ok(())
            })
            .await;
    }

    async fn search_entities_by_type(&self, entity_type: &str) -> Vec<Entity> {
        let query = SearchQuery {
            lifecycle: LifecycleFilter::Active,
            entity_type: Some(entity_type.to_string()),
            limit: u32::MAX,
            sort: SortBy::Name,
            ..SearchQuery::default()
        };
        let enable_fts = self.enable_fts;
        self.db
            .run(move |conn| search_entities_sql(conn, &query, enable_fts))
            .await
            .unwrap_or_default()
    }

    async fn search_entities_by_name(&self, name: &str) -> Vec<Entity> {
        let query = SearchQuery {
            lifecycle: LifecycleFilter::Active,
            text: Some(name.to_string()),
            limit: u32::MAX,
            sort: SortBy::Name,
            ..SearchQuery::default()
        };
        let enable_fts = self.enable_fts;
        self.db
            .run(move |conn| search_entities_sql(conn, &query, enable_fts))
            .await
            .unwrap_or_default()
    }

    async fn all_entities(&self) -> Vec<Entity> {
        let query = SearchQuery {
            lifecycle: LifecycleFilter::Active,
            limit: u32::MAX,
            sort: SortBy::Name,
            ..SearchQuery::default()
        };
        let enable_fts = self.enable_fts;
        self.db
            .run(move |conn| search_entities_sql(conn, &query, enable_fts))
            .await
            .unwrap_or_default()
    }

    async fn all_relationships(&self) -> Vec<Relationship> {
        self.db
            .run(move |conn| {
                let mut stmt = conn.prepare(&format!(
                    "SELECT {RELATIONSHIP_COLS} FROM relationships ORDER BY created_at ASC"
                ))?;
                let rows = stmt.query_map([], read_raw_relationship)?;
                let mut out = Vec::new();
                for row in rows {
                    out.push(raw_relationship_to_relationship(&row?)?);
                }
                Ok(out)
            })
            .await
            .unwrap_or_default()
    }

    async fn insert_relationship(&self, rel: &Relationship) -> RelationshipId {
        let id_str = relationship_id_str(&rel.id);
        let raw = match relationship_to_raw(rel, "ai", None) {
            Ok(raw) => raw,
            Err(e) => {
                tracing::warn!(error = %e, "world store: failed to serialise relationship");
                return rel.id;
            }
        };
        let snapshot = serde_json::to_value(rel).unwrap_or_default();
        let changed_by = "ai".to_string();
        let result_id = rel.id;
        let version = rel.version;
        let updated_at = rel.updated_at;
        let warn_id = id_str.clone();
        if let Err(e) = self
            .db
            .run_mut(move |conn| {
                let existing = get_raw_relationship(conn, &id_str)?;
                if existing.is_some() {
                    update_relationship_row(conn, &raw)?;
                } else {
                    insert_relationship_row(conn, &raw)?;
                }
                append_relationship_history(
                    conn,
                    &id_str,
                    version,
                    Operation::Create,
                    &snapshot,
                    &changed_by,
                    None,
                    updated_at,
                )?;
                Ok(())
            })
            .await
        {
            tracing::warn!(error = %e, id = %warn_id, "world store: insert_relationship failed");
        }
        result_id
    }

    async fn update_relationship(&self, rel: &Relationship) {
        let id_str = relationship_id_str(&rel.id);
        let raw = match relationship_to_raw(rel, "ai", None) {
            Ok(raw) => raw,
            Err(e) => {
                tracing::warn!(error = %e, "world store: failed to serialise relationship update");
                return;
            }
        };
        let snapshot = serde_json::to_value(rel).unwrap_or_default();
        let changed_by = "ai".to_string();
        let version = rel.version;
        let updated_at = rel.updated_at;
        let warn_id = id_str.clone();
        if let Err(e) = self
            .db
            .run_mut(move |conn| {
                update_relationship_row(conn, &raw)?;
                append_relationship_history(
                    conn,
                    &id_str,
                    version,
                    Operation::Update,
                    &snapshot,
                    &changed_by,
                    None,
                    updated_at,
                )?;
                Ok(())
            })
            .await
        {
            tracing::warn!(error = %e, id = %warn_id, "world store: update_relationship failed");
        }
    }

    async fn get_relationship(&self, id: &RelationshipId) -> Option<Relationship> {
        let id_str = relationship_id_str(id);
        self.db
            .run(move |conn| {
                let raw = get_raw_relationship(conn, &id_str)?;
                raw.map(|r| raw_relationship_to_relationship(&r))
                    .transpose()
            })
            .await
            .ok()
            .flatten()
    }

    async fn get_relationships_for_entity(&self, entity_id: &EntityId) -> Vec<Relationship> {
        let id_str = entity_id_str(entity_id);
        self.db
            .run(move |conn| {
                let mut stmt = conn.prepare(&format!(
                    "SELECT {RELATIONSHIP_COLS} FROM relationships \
                     WHERE source_id = ?1 OR target_id = ?1"
                ))?;
                let rows = stmt.query_map(params![id_str], read_raw_relationship)?;
                let mut out = Vec::new();
                for row in rows {
                    out.push(raw_relationship_to_relationship(&row?)?);
                }
                Ok(out)
            })
            .await
            .unwrap_or_default()
    }

    async fn get_relationships_between(
        &self,
        source: &EntityId,
        target: &EntityId,
    ) -> Vec<Relationship> {
        let source_str = entity_id_str(source);
        let target_str = entity_id_str(target);
        self.db
            .run(move |conn| {
                let mut stmt = conn.prepare(&format!(
                    "SELECT {RELATIONSHIP_COLS} FROM relationships \
                     WHERE source_id = ?1 AND target_id = ?2"
                ))?;
                let rows =
                    stmt.query_map(params![source_str, target_str], read_raw_relationship)?;
                let mut out = Vec::new();
                for row in rows {
                    out.push(raw_relationship_to_relationship(&row?)?);
                }
                Ok(out)
            })
            .await
            .unwrap_or_default()
    }

    async fn get_neighbors(
        &self,
        entity_id: &EntityId,
        rel_types: Option<&[&str]>,
    ) -> Vec<(Relationship, Entity)> {
        let id_str = entity_id_str(entity_id);
        let rel_types_owned: Option<Vec<String>> =
            rel_types.map(|types| types.iter().map(|s| s.to_string()).collect());
        let rels = self
            .db
            .run(move |conn| {
                let mut sql =
                    format!("SELECT {RELATIONSHIP_COLS} FROM relationships WHERE source_id = ?1");
                let mut params: Vec<rusqlite::types::Value> =
                    vec![rusqlite::types::Value::Text(id_str.clone())];
                if let Some(types) = &rel_types_owned {
                    let placeholders = std::iter::repeat_n("?", types.len())
                        .collect::<Vec<_>>()
                        .join(",");
                    sql.push_str(&format!(" AND relationship_type IN ({placeholders})"));
                    for t in types {
                        params.push(rusqlite::types::Value::Text(t.clone()));
                    }
                }
                let mut stmt = conn.prepare(&sql)?;
                let rows = stmt.query_map(
                    rusqlite::params_from_iter(params.iter()),
                    read_raw_relationship,
                )?;
                let mut out = Vec::new();
                for row in rows {
                    out.push(raw_relationship_to_relationship(&row?)?);
                }
                Ok(out)
            })
            .await
            .unwrap_or_default();

        let mut out = Vec::new();
        for rel in &rels {
            let neighbor_id = entity_id_str(&rel.target_id);
            let target_raw = self
                .db
                .run(move |conn| {
                    let raw = get_raw_entity(conn, &neighbor_id)?;
                    raw.map(|r| raw_entity_to_entity(&r)).transpose()
                })
                .await
                .ok()
                .flatten();
            if let Some(target) = target_raw.filter(|t| t.lifecycle == EntityLifecycle::Active) {
                out.push((rel.clone(), target));
            }
        }
        out
    }

    async fn get_incoming_neighbors(
        &self,
        entity_id: &EntityId,
        rel_types: Option<&[&str]>,
    ) -> Vec<(Relationship, Entity)> {
        let id_str = entity_id_str(entity_id);
        let rel_types_owned: Option<Vec<String>> =
            rel_types.map(|types| types.iter().map(|s| s.to_string()).collect());
        let rels = self
            .db
            .run(move |conn| {
                let mut sql =
                    format!("SELECT {RELATIONSHIP_COLS} FROM relationships WHERE target_id = ?1");
                let mut params: Vec<rusqlite::types::Value> =
                    vec![rusqlite::types::Value::Text(id_str.clone())];
                if let Some(types) = &rel_types_owned {
                    let placeholders = std::iter::repeat_n("?", types.len())
                        .collect::<Vec<_>>()
                        .join(",");
                    sql.push_str(&format!(" AND relationship_type IN ({placeholders})"));
                    for t in types {
                        params.push(rusqlite::types::Value::Text(t.clone()));
                    }
                }
                let mut stmt = conn.prepare(&sql)?;
                let rows = stmt.query_map(
                    rusqlite::params_from_iter(params.iter()),
                    read_raw_relationship,
                )?;
                let mut out = Vec::new();
                for row in rows {
                    out.push(raw_relationship_to_relationship(&row?)?);
                }
                Ok(out)
            })
            .await
            .unwrap_or_default();

        let mut out = Vec::new();
        for rel in &rels {
            let neighbor_id = entity_id_str(&rel.source_id);
            let source_raw = self
                .db
                .run(move |conn| {
                    let raw = get_raw_entity(conn, &neighbor_id)?;
                    raw.map(|r| raw_entity_to_entity(&r)).transpose()
                })
                .await
                .ok()
                .flatten();
            if let Some(source) = source_raw.filter(|s| s.lifecycle == EntityLifecycle::Active) {
                out.push((rel.clone(), source));
            }
        }
        out
    }

    async fn entity_count(&self) -> usize {
        self.db
            .run(move |conn| {
                let n: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM entities WHERE lifecycle = 'Active'",
                    [],
                    |row| row.get(0),
                )?;
                Ok(n as usize)
            })
            .await
            .unwrap_or_default()
    }

    async fn relationship_count(&self) -> usize {
        self.db
            .run(move |conn| {
                let n: i64 =
                    conn.query_row("SELECT COUNT(*) FROM relationships", [], |row| row.get(0))?;
                Ok(n as usize)
            })
            .await
            .unwrap_or_default()
    }

    async fn clear(&self) {
        let enable_fts = self.enable_fts;
        let _ = self
            .db
            .run_mut(move |conn| {
                let tx = conn.transaction()?;
                tx.execute_batch(
                    "DELETE FROM entity_observations; \
                     DELETE FROM document_entities; \
                     DELETE FROM document_content; \
                     DELETE FROM relationships; \
                     DELETE FROM relationship_history; \
                     DELETE FROM entities; \
                     DELETE FROM entity_history;",
                )?;
                if enable_fts {
                    tx.execute_batch("DELETE FROM entities_fts; DELETE FROM document_fts;")?;
                }
                tx.commit()?;
                Ok(())
            })
            .await;
    }
}

// ── Goals / lessons persistence ──────────────────────────────────────────────

impl SqliteWorldModelStore {
    /// Upsert a goal. The durable row keeps its own version; when the caller
    /// supplies a non-zero `version` and it does not match the stored value, a
    /// [`WorldStoreError::VersionConflict`] is returned.
    ///
    /// A `goal` entity row (same UUID) is upserted alongside so the goal is
    /// visible in the property graph.
    pub async fn upsert_goal(&self, goal: StoredGoal) -> WorldStoreResult<StoredGoal> {
        // The goal id must be a valid UUID because the goal is mirrored as an
        // `entities` row with the same id. Validate before any write so an
        // invalid id cannot leave an orphaned `goals` row behind.
        str_entity_id(&goal.goal_id)?;
        let enable_fts = self.enable_fts;
        let id_str = goal.goal_id.clone();
        let expected = goal.version;
        let stored = self
            .db
            .run_mut(move |conn| {
                let existing = get_raw_goal(conn, &id_str)?;
                let (version, created_at, updated_at) = match &existing {
                    Some(raw) => {
                        check_version(raw.version, expected, &id_str)?;
                        (raw.version + 1, raw.created_at, Timestamp::now().as_secs())
                    }
                    None => (1, Timestamp::now().as_secs(), Timestamp::now().as_secs()),
                };
                conn.execute(
                    "INSERT INTO goals \
                     (goal_id, goal_type, description, priority, status, parent, children_json, \
                      dependencies_json, version, retry_count, progress_pct, source, outcome, \
                      created_at, updated_at, metadata_json) \
                     VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16) \
                     ON CONFLICT(goal_id) DO UPDATE SET \
                     goal_type = excluded.goal_type, description = excluded.description, \
                     priority = excluded.priority, status = excluded.status, \
                     parent = excluded.parent, children_json = excluded.children_json, \
                     dependencies_json = excluded.dependencies_json, version = excluded.version, \
                     retry_count = excluded.retry_count, progress_pct = excluded.progress_pct, \
                     source = excluded.source, outcome = excluded.outcome, \
                     updated_at = excluded.updated_at, metadata_json = excluded.metadata_json",
                    params![
                        id_str,
                        goal.goal_type,
                        goal.description,
                        goal.priority,
                        goal.status,
                        goal.parent,
                        to_json(&goal.children)?,
                        to_json(&goal.dependencies)?,
                        version as i64,
                        goal.retry_count as i64,
                        goal.progress_pct,
                        goal.source,
                        goal.outcome,
                        created_at,
                        updated_at,
                        to_json(&goal.metadata)?,
                    ],
                )?;

                let entity = Entity {
                    id: str_entity_id(&id_str)?,
                    entity_type: "goal".to_string(),
                    name: goal.description,
                    properties: HashMap::new(),
                    importance: 0.5,
                    confidence: 0.5,
                    created_at: Timestamp::from_secs(created_at),
                    updated_at: Timestamp::from_secs(updated_at),
                    version: 1,
                    lifecycle: EntityLifecycle::Active,
                    metadata: HashMap::new(),
                };
                let raw = entity_to_raw(&entity, "runtime", None)?;
                let existing_entity = get_raw_entity(conn, &id_str)?;
                if existing_entity.is_some() {
                    update_entity_row(conn, &raw)?;
                } else {
                    insert_entity_row(conn, &raw)?;
                }
                sync_entity_fts(conn, enable_fts, &raw)?;

                Ok(StoredGoal {
                    goal_id: id_str,
                    goal_type: goal.goal_type,
                    description: entity.name,
                    priority: goal.priority,
                    status: goal.status,
                    parent: goal.parent,
                    children: goal.children,
                    dependencies: goal.dependencies,
                    version,
                    retry_count: goal.retry_count,
                    progress_pct: goal.progress_pct,
                    source: goal.source,
                    outcome: goal.outcome,
                    created_at: Timestamp::from_secs(created_at),
                    updated_at: Timestamp::from_secs(updated_at),
                    metadata: goal.metadata,
                })
            })
            .await?;
        Ok(stored)
    }

    /// Fetch a goal by id.
    pub async fn get_goal(&self, goal_id: &str) -> WorldStoreResult<Option<StoredGoal>> {
        let id_str = goal_id.to_string();
        self.db
            .run(move |conn| {
                let raw = get_raw_goal(conn, &id_str)?;
                raw.map(|r| raw_goal_to_goal(&r)).transpose()
            })
            .await
    }

    /// All goals, most recently updated first.
    pub async fn all_goals(&self) -> WorldStoreResult<Vec<StoredGoal>> {
        self.db
            .run(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT goal_id, goal_type, description, priority, status, parent, \
                     children_json, dependencies_json, version, retry_count, progress_pct, \
                     source, outcome, created_at, updated_at, metadata_json \
                     FROM goals ORDER BY updated_at DESC",
                )?;
                let rows = stmt.query_map([], read_raw_goal)?;
                let mut out = Vec::new();
                for row in rows {
                    out.push(raw_goal_to_goal(&row?)?);
                }
                Ok(out)
            })
            .await
    }

    /// Upsert a lesson (lessons are immutable once written; re-writing replaces).
    pub async fn upsert_lesson(&self, lesson: StoredLesson) -> WorldStoreResult<StoredLesson> {
        let id_str = lesson.lesson_id.clone();
        self.db
            .run_mut(move |conn| {
                conn.execute(
                    "INSERT INTO lessons \
                     (lesson_id, category, content, source, confidence, created_at) \
                     VALUES (?1,?2,?3,?4,?5,?6) \
                     ON CONFLICT(lesson_id) DO UPDATE SET \
                     category = excluded.category, content = excluded.content, \
                     source = excluded.source, confidence = excluded.confidence",
                    params![
                        id_str,
                        lesson.category,
                        lesson.content,
                        lesson.source,
                        lesson.confidence,
                        lesson.created_at.as_secs(),
                    ],
                )?;
                Ok(lesson)
            })
            .await
    }

    /// Fetch a lesson by id.
    pub async fn get_lesson(&self, lesson_id: &str) -> WorldStoreResult<Option<StoredLesson>> {
        let id_str = lesson_id.to_string();
        self.db
            .run(move |conn| {
                conn.query_row(
                    "SELECT lesson_id, category, content, source, confidence, created_at \
                     FROM lessons WHERE lesson_id = ?1",
                    params![id_str],
                    |row| {
                        Ok(StoredLesson {
                            lesson_id: row.get(0)?,
                            category: row.get(1)?,
                            content: row.get(2)?,
                            source: row.get(3)?,
                            confidence: row.get(4)?,
                            created_at: Timestamp::from_secs(row.get(5)?),
                        })
                    },
                )
                .optional()
                .map_err(Into::into)
            })
            .await
    }

    /// All lessons, newest first.
    pub async fn all_lessons(&self) -> WorldStoreResult<Vec<StoredLesson>> {
        self.db
            .run(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT lesson_id, category, content, source, confidence, created_at \
                     FROM lessons ORDER BY created_at DESC",
                )?;
                let rows = stmt.query_map([], |row| {
                    Ok(StoredLesson {
                        lesson_id: row.get(0)?,
                        category: row.get(1)?,
                        content: row.get(2)?,
                        source: row.get(3)?,
                        confidence: row.get(4)?,
                        created_at: Timestamp::from_secs(row.get(5)?),
                    })
                })?;
                let mut out = Vec::new();
                for row in rows {
                    out.push(row?);
                }
                Ok(out)
            })
            .await
    }

    /// Back up the database (via `VACUUM INTO`) and the documents directory to
    /// `dest_dir`, writing a [`BackupManifest`] (`backup.json`) and rotating
    /// snapshots down to the configured `backup_count`.
    pub async fn backup(&self, dest_dir: &Path) -> WorldStoreResult<BackupManifest> {
        let backup_count = self.backup_count;
        let documents_dir = self.documents_dir.clone();
        let dest_dir = dest_dir.to_path_buf();

        let db_file = format!("{}.db", Timestamp::now().as_secs());
        std::fs::create_dir_all(&dest_dir).map_err(|e| {
            WorldStoreError::Io(format!("create backup dir {}: {e}", dest_dir.display()))
        })?;
        let snapshot_path = dest_dir.join(&db_file);
        if snapshot_path.exists() {
            std::fs::remove_file(&snapshot_path)
                .map_err(|e| WorldStoreError::Io(format!("remove stale snapshot: {e}")))?;
        }

        let snapshot_for_closure = snapshot_path.clone();
        self.db
            .run_mut(move |conn| {
                let quoted = snapshot_for_closure
                    .display()
                    .to_string()
                    .replace('\'', "''");
                conn.execute_batch(&format!("VACUUM INTO '{quoted}';"))
                    .map_err(|e| WorldStoreError::Storage(format!("vacuum into backup: {e}")))?;
                Ok(())
            })
            .await?;

        let db_bytes = std::fs::read(&snapshot_path)
            .map_err(|e| WorldStoreError::Io(format!("read snapshot: {e}")))?;
        let db_sha256 = sha256_hex(&db_bytes);
        let documents_sha256 = sha256_tree(&documents_dir);

        let manifest = BackupManifest {
            created_at: Timestamp::now().as_secs(),
            db_file,
            db_sha256,
            documents_sha256,
        };
        let manifest_json = serde_json::to_string_pretty(&manifest)
            .map_err(|e| WorldStoreError::Validation(e.to_string()))?;
        std::fs::write(dest_dir.join("backup.json"), manifest_json)
            .map_err(|e| WorldStoreError::Io(format!("write backup manifest: {e}")))?;

        rotate_snapshots(&dest_dir, backup_count)?;
        Ok(manifest)
    }

    /// Restore from a backup directory containing `backup.json` and the
    /// snapshot DB. Verifies the snapshot checksum and runs `integrity_check`
    /// before swapping it into place.
    pub async fn restore(&self, from_dir: &Path) -> WorldStoreResult<RestoreReport> {
        let manifest: BackupManifest = {
            let path = from_dir.join("backup.json");
            let text = std::fs::read_to_string(&path).map_err(|e| {
                WorldStoreError::Io(format!("read backup manifest {}: {e}", path.display()))
            })?;
            serde_json::from_str(&text)
                .map_err(|e| WorldStoreError::Validation(format!("parse backup manifest: {e}")))?
        };
        let snapshot_path = from_dir.join(&manifest.db_file);
        let bytes = std::fs::read(&snapshot_path)
            .map_err(|e| WorldStoreError::Io(format!("read snapshot: {e}")))?;
        if sha256_hex(&bytes) != manifest.db_sha256 {
            return Err(WorldStoreError::Migration(
                "backup snapshot checksum mismatch".into(),
            ));
        }

        // Verify the backup is a valid, intact database without mutating it.
        let verify = Db::open_verify_only(&snapshot_path, false)?;
        let integrity = verify
            .run(move |conn| {
                let row: String = conn.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
                Ok(row)
            })
            .await?;
        drop(verify);

        self.db.swap_file(&snapshot_path).await?;

        let (entities, relationships) = self
            .db
            .run(move |conn| {
                let e: i64 =
                    conn.query_row("SELECT COUNT(*) FROM entities", [], |row| row.get(0))?;
                let r: i64 =
                    conn.query_row("SELECT COUNT(*) FROM relationships", [], |row| row.get(0))?;
                Ok((e as u64, r as u64))
            })
            .await?;

        let documents_match = sha256_tree(&self.documents_dir) == manifest.documents_sha256;
        Ok(RestoreReport {
            integrity,
            entities,
            relationships,
            documents_match,
        })
    }

    /// Record a state-changes payload on an observation and advance its
    /// understanding pipeline state.
    pub async fn mark_observation_processed(
        &self,
        id: &ObservationId,
        processed: ObservationProcessState,
        state_changes: Option<&serde_json::Value>,
    ) -> WorldStoreResult<()> {
        let id_str = id.to_string();
        let state_changes_json = state_changes.map(value_to_json).transpose()?;
        self.db
            .run_mut(move |conn| {
                let updated = conn.execute(
                    "UPDATE observations SET processed = ?2, state_changes_json = ?3 \
                     WHERE id = ?1",
                    params![id_str, processed.as_u8(), state_changes_json],
                )?;
                if updated == 0 {
                    return Err(WorldStoreError::ObservationNotFound(id_str));
                }
                Ok(())
            })
            .await
    }

    /// Link an entity to an observation.
    pub async fn link_entity_observation(
        &self,
        entity_id: &EntityId,
        observation_id: &ObservationId,
    ) -> WorldStoreResult<()> {
        let eid = entity_id_str(entity_id);
        let oid = observation_id.to_string();
        self.db
            .run_mut(move |conn| {
                conn.execute(
                    "INSERT OR IGNORE INTO entity_observations (entity_id, observation_id) \
                     VALUES (?1,?2)",
                    params![eid, oid],
                )?;
                Ok(())
            })
            .await
    }

    /// Migrate the companion host's `world_model.json` into the World Store.
    ///
    /// Idempotent: skipped when the migration marker is set or entities already
    /// exist. The source file is copied to a timestamped `.bak` before import.
    pub async fn migrate_from_json(&self, source: &Path) -> WorldStoreResult<MigrationReport> {
        let enable_fts = self.enable_fts;
        if !source.exists() {
            return Ok(MigrationReport {
                skipped: true,
                entities_imported: 0,
                relationships_imported: 0,
                source_backup: None,
                source_file: Some(source.to_path_buf()),
            });
        }

        let done = self.migration_done().await?;
        if done {
            return Ok(MigrationReport {
                skipped: true,
                entities_imported: 0,
                relationships_imported: 0,
                source_backup: None,
                source_file: Some(source.to_path_buf()),
            });
        }

        let text = std::fs::read_to_string(source)
            .map_err(|e| WorldStoreError::Migration(format!("read {}: {e}", source.display())))?;
        let model: crate::types::JsonWorldModel = serde_json::from_str(&text)
            .map_err(|e| WorldStoreError::Migration(format!("parse {}: {e}", source.display())))?;

        let backup_path = PathBuf::from(format!(
            "{}.bak-{}",
            source.display(),
            Timestamp::now().as_secs()
        ));
        std::fs::copy(source, &backup_path).map_err(|e| {
            WorldStoreError::Migration(format!("back up {}: {e}", source.display()))
        })?;

        let entities = model.entities.clone();
        let relationships = model.relationships.clone();
        let (entities_imported, relationships_imported) = self
            .db
            .run_mut(move |conn| {
                let count: i64 =
                    conn.query_row("SELECT COUNT(*) FROM entities", [], |row| row.get(0))?;
                if count > 0 {
                    return Ok((0usize, 0usize));
                }
                let mut entity_ids: HashMap<String, EntityId> = HashMap::new();
                for entity in &entities {
                    let id_str = entity_id_str(&entity.id);
                    entity_ids.insert(entity.name.clone(), entity.id);
                    let raw =
                        entity_to_raw(entity, "migration", Some("migration:world_model.json"))?;
                    insert_entity_row(conn, &raw)?;
                    let snapshot = serde_json::to_value(entity)
                        .map_err(|e| WorldStoreError::Validation(e.to_string()))?;
                    append_entity_history(
                        conn,
                        &id_str,
                        entity.version,
                        Operation::Create,
                        &snapshot,
                        "migration",
                        Some("migration:world_model.json"),
                        entity.updated_at,
                        None,
                    )?;
                    sync_entity_fts(conn, enable_fts, &raw)?;
                }
                for rel in &relationships {
                    let raw =
                        relationship_to_raw(rel, "migration", Some("migration:world_model.json"))?;
                    let id_str = relationship_id_str(&rel.id);
                    insert_relationship_row(conn, &raw)?;
                    let snapshot = serde_json::to_value(rel)
                        .map_err(|e| WorldStoreError::Validation(e.to_string()))?;
                    append_relationship_history(
                        conn,
                        &id_str,
                        rel.version,
                        Operation::Create,
                        &snapshot,
                        "migration",
                        Some("migration:world_model.json"),
                        rel.updated_at,
                    )?;
                }
                let _ = entity_ids;
                conn.execute(
                    "INSERT INTO meta (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO NOTHING",
                    params![META_MIGRATION_DONE, "1"],
                )?;
                Ok((entities.len(), relationships.len()))
            })
            .await?;

        if entities_imported == 0 && relationships_imported == 0 && !model.entities.is_empty() {
            // Entities already present (concurrent import); leave marker alone.
            std::fs::remove_file(&backup_path).ok();
        }

        Ok(MigrationReport {
            skipped: false,
            entities_imported,
            relationships_imported,
            source_backup: Some(backup_path),
            source_file: Some(source.to_path_buf()),
        })
    }
}

// ── Goal row helpers ─────────────────────────────────────────────────────────

fn get_raw_goal(conn: &Connection, id: &str) -> WorldStoreResult<Option<RawGoal>> {
    conn.query_row(
        "SELECT goal_id, goal_type, description, priority, status, parent, children_json, \
         dependencies_json, version, retry_count, progress_pct, source, outcome, created_at, \
         updated_at, metadata_json \
         FROM goals WHERE goal_id = ?1",
        params![id],
        read_raw_goal,
    )
    .optional()
    .map_err(Into::into)
}

/// Recursively hash every file under `dir` (sorted paths) into one SHA-256.
fn sha256_tree(dir: &Path) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    let mut paths: Vec<PathBuf> = Vec::new();
    collect_files(dir, &mut paths);
    paths.sort();
    for path in paths {
        if let Ok(bytes) = std::fs::read(&path) {
            hasher.update(b"file:");
            hasher.update(bytes);
        }
    }
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, out);
        } else {
            out.push(path);
        }
    }
}

fn rotate_snapshots(dest_dir: &Path, keep: u32) -> WorldStoreResult<()> {
    let mut snaps: Vec<PathBuf> = std::fs::read_dir(dest_dir)
        .map_err(|e| WorldStoreError::Io(format!("list backup dir: {e}")))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().map(|e| e == "db").unwrap_or(false))
        .collect();
    snaps.sort();
    if snaps.len() as u32 > keep {
        let to_remove = snaps.len() as u32 - keep;
        for path in snaps.into_iter().take(to_remove as usize) {
            std::fs::remove_file(&path)
                .map_err(|e| WorldStoreError::Io(format!("rotate snapshot {:?}: {e}", path)))?;
        }
    }
    Ok(())
}
