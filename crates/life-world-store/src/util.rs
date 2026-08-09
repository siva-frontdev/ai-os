//! Internal helpers shared across the store implementation.

use std::collections::HashMap;

use memory_core::wm::{EntityId, EntityLifecycle, RelationshipId};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::error::{WorldStoreError, WorldStoreResult};

/// Serialize a value to a JSON string (used for the JSON `properties`/`metadata`
/// columns and history snapshots).
pub(crate) fn to_json<T: Serialize>(value: &T) -> WorldStoreResult<String> {
    serde_json::to_string(value).map_err(|e| WorldStoreError::Validation(e.to_string()))
}

/// Deserialize a JSON string column back into a value.
pub(crate) fn from_json<T: DeserializeOwned>(text: &str) -> WorldStoreResult<T> {
    serde_json::from_str(text)
        .map_err(|e| WorldStoreError::Validation(format!("failed to parse stored JSON: {e}")))
}

/// SHA-256 of a byte slice as lowercase hex (entity `checksum`, document
/// checksums, backup manifest).
pub(crate) fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(data);
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

/// Format an entity id as a stable string key.
pub(crate) fn entity_id_str(id: &EntityId) -> String {
    id.as_uuid().to_string()
}

/// Parse an entity id from a string key.
pub(crate) fn str_entity_id(s: &str) -> WorldStoreResult<EntityId> {
    uuid::Uuid::parse_str(s)
        .map(EntityId::from_uuid)
        .map_err(|e| WorldStoreError::Validation(format!("invalid entity id {s}: {e}")))
}

/// Format a relationship id as a stable string key.
pub(crate) fn relationship_id_str(id: &RelationshipId) -> String {
    id.to_string()
}

/// Parse a relationship id from a string key.
pub(crate) fn str_relationship_id(s: &str) -> WorldStoreResult<RelationshipId> {
    uuid::Uuid::parse_str(s)
        .map(RelationshipId::from_uuid)
        .map_err(|e| WorldStoreError::Validation(format!("invalid relationship id {s}: {e}")))
}

/// Serialize a lifecycle to its stored text form.
pub(crate) fn lifecycle_str(lifecycle: &EntityLifecycle) -> &'static str {
    match lifecycle {
        EntityLifecycle::Active => "Active",
        EntityLifecycle::Archived => "Archived",
        EntityLifecycle::Forgotten => "Forgotten",
    }
}

/// Parse a lifecycle from its stored text form.
pub(crate) fn str_lifecycle(s: &str) -> WorldStoreResult<EntityLifecycle> {
    match s {
        "Active" => Ok(EntityLifecycle::Active),
        "Archived" => Ok(EntityLifecycle::Archived),
        "Forgotten" => Ok(EntityLifecycle::Forgotten),
        other => Err(WorldStoreError::Validation(format!(
            "invalid lifecycle {other}"
        ))),
    }
}

/// The typed entity catalogue (schema §6). New writes are normalised against
/// this list; free-form types are preserved verbatim and tagged in
/// `metadata.original_type`.
pub(crate) const ENTITY_TYPE_CATALOGUE: &[&str] = &[
    "user",
    "person",
    "contact",
    "company",
    "organization",
    "project",
    "goal",
    "mission",
    "job",
    "lead",
    "event",
    "document",
    "knowledge",
    "concept",
    "observation",
    "email",
    "message",
    "meeting",
    "task",
    "skill",
    "resource",
    "location",
    "product",
    "topic",
];

/// Maximum length of an `entity_type` string (RFC-0008 §Security "length
/// limits"). Bound is generous; catalogue types are all well under it.
pub(crate) const MAX_ENTITY_TYPE_LEN: usize = 128;

/// Validate and normalise an `entity_type` for a new write.
///
/// RFC-0008 §Security requires `entity_type` input validation (catalogue +
/// length limits) at the store boundary. An empty or over-length type is
/// rejected with [`WorldStoreError::InvalidEntityType`]. Otherwise the type is
/// stored verbatim: canonical catalogue values unchanged, free-form values
/// preserved and recorded in `metadata.original_type` so no information is lost
/// (ADR-0008 Decision 1).
pub(crate) fn normalize_entity_type(
    entity_type: &str,
    metadata: &mut HashMap<String, String>,
) -> WorldStoreResult<String> {
    let trimmed = entity_type.trim();
    if trimmed.is_empty() {
        return Err(WorldStoreError::InvalidEntityType(
            "entity_type must not be empty".to_string(),
        ));
    }
    if trimmed.chars().count() > MAX_ENTITY_TYPE_LEN {
        return Err(WorldStoreError::InvalidEntityType(format!(
            "entity_type exceeds {MAX_ENTITY_TYPE_LEN} characters"
        )));
    }
    if ENTITY_TYPE_CATALOGUE.contains(&trimmed) {
        Ok(trimmed.to_string())
    } else {
        metadata.insert("original_type".to_string(), trimmed.to_string());
        Ok(trimmed.to_string())
    }
}

/// Build a safe FTS5 MATCH expression from free-form user text.
///
/// Tokens are double-quoted (and embedded quotes escaped) so arbitrary input
/// cannot inject FTS5 query operators. Multiple tokens are AND-ed.
pub(crate) fn fts_query(text: &str) -> String {
    text.split_whitespace()
        .filter(|t| !t.is_empty())
        .map(|t| format!("\"{}\"", t.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" ")
}
