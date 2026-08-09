//! The backend-neutral World Store API (RFC-0008 §Interfaces).
//!
//! Both the AI (cognitive loop) and the user (Open Space / companion controls)
//! speak to the durable world through this surface. The only difference between
//! the two is the [`WriteContext`] (actor + provenance) and the authorization
//! path that gates the call.

use async_trait::async_trait;
use memory_core::wm::{Entity, EntityId, Relationship, RelationshipId};

use crate::error::WorldStoreResult;
use crate::types::{
    CreateDocument, CreateEntity, CreateRelationship, Document, DocumentId, HistoryEntry,
    Observation, ObservationId, SearchQuery, UpdateDocument, UpdateEntity, UpdateRelationship,
    WriteContext,
};

/// Backend-neutral durable World Store.
#[async_trait]
pub trait WorldStore: Send + Sync + std::fmt::Debug {
    // ---- Entities ----

    /// Create an entity, returning the stored canonical [`Entity`].
    async fn create_entity(&self, req: CreateEntity) -> WorldStoreResult<Entity>;

    /// Fetch an entity by id (any lifecycle).
    async fn get_entity(&self, id: &EntityId) -> WorldStoreResult<Option<Entity>>;

    /// Update an entity with optimistic concurrency; a stale
    /// `expected_version` yields [`crate::WorldStoreError::VersionConflict`].
    async fn update_entity(&self, req: UpdateEntity) -> WorldStoreResult<Entity>;

    /// Soft delete an entity (lifecycle → `Archived`).
    async fn archive_entity(
        &self,
        id: &EntityId,
        expected_version: u64,
        reason: &str,
        ctx: &WriteContext,
    ) -> WorldStoreResult<()>;

    /// Restore an archived entity back to `Active`.
    async fn restore_entity(
        &self,
        id: &EntityId,
        expected_version: u64,
        ctx: &WriteContext,
    ) -> WorldStoreResult<()>;

    /// Delete an entity. `hard == false` soft-archives and retains history;
    /// `hard == true` is the explicit privileged operation that removes the row
    /// and its history.
    async fn delete_entity(
        &self,
        id: &EntityId,
        expected_version: u64,
        hard: bool,
        ctx: &WriteContext,
    ) -> WorldStoreResult<()>;

    // ---- Relationships ----

    /// Create a relationship, returning the stored canonical [`Relationship`].
    async fn create_relationship(&self, req: CreateRelationship) -> WorldStoreResult<Relationship>;

    /// Fetch a relationship by id.
    async fn get_relationship(&self, id: &RelationshipId)
    -> WorldStoreResult<Option<Relationship>>;

    /// Update a relationship with optimistic concurrency.
    async fn update_relationship(&self, req: UpdateRelationship) -> WorldStoreResult<Relationship>;

    /// Delete a relationship (history retained).
    async fn delete_relationship(
        &self,
        id: &RelationshipId,
        expected_version: u64,
        ctx: &WriteContext,
    ) -> WorldStoreResult<()>;

    // ---- Search ----

    /// Full-text + filtered entity search.
    async fn search_entities(&self, query: &SearchQuery) -> WorldStoreResult<Vec<Entity>>;

    /// Full-text + filtered document search.
    async fn search_documents(&self, query: &SearchQuery) -> WorldStoreResult<Vec<Document>>;

    // ---- Observations ----

    /// Persist an observation durably.
    async fn record_observation(&self, obs: Observation) -> WorldStoreResult<ObservationId>;

    /// Fetch a stored observation.
    async fn get_observation(&self, id: &ObservationId) -> WorldStoreResult<Option<Observation>>;

    // ---- History ----

    /// Ordered change log for an entity (oldest first).
    async fn get_history(&self, id: &EntityId) -> WorldStoreResult<Vec<HistoryEntry>>;

    // ---- Documents ----

    /// Create a document (blob + metadata + extracted text).
    async fn create_document(&self, req: CreateDocument) -> WorldStoreResult<Document>;

    /// Fetch a document by id.
    async fn get_document(&self, id: &DocumentId) -> WorldStoreResult<Option<Document>>;

    /// Update a document with optimistic concurrency.
    async fn update_document(&self, req: UpdateDocument) -> WorldStoreResult<Document>;

    /// Soft delete a document.
    async fn archive_document(
        &self,
        id: &DocumentId,
        expected_version: u64,
        ctx: &WriteContext,
    ) -> WorldStoreResult<()>;
}
