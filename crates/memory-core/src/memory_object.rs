use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::relationship::Relationship;
use crate::types::*;

/// The universal memory record for the AI-native OS Memory Platform.
///
/// Every piece of information stored in the memory subsystem is a `MemoryObject`.
/// Objects are immutable once created — updates create new versions with a new checksum.
/// The structure follows a versioned, append-only design for auditability and traceability.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryObject {
    // -- Identity --
    /// Unique identifier (UUID v7, time-sortable).
    pub id: MemoryId,
    /// Monotonic version counter. Starts at 1 for new objects.
    pub version: Version,

    // -- Origin --
    /// When this version was created (UNIX epoch nanoseconds).
    pub timestamp: Timestamp,
    /// Identifier of the agent or module that created this object.
    pub created_by: String,
    /// The origin source classification.
    pub source: MemorySource,

    // -- Classification --
    /// The cognitive memory type (Working, Episodic, Semantic, Knowledge).
    pub memory_type: MemoryType,
    /// The physical storage tier.
    pub tier: MemoryTier,
    /// MIME-like content type descriptor (e.g. "text/markdown", "application/json").
    pub content_type: String,

    // -- Value --
    /// Priority level 0-255. Higher = more important.
    pub priority: u8,
    /// Importance score 0.0-1.0. Computed by the learning engine.
    pub importance: f32,
    /// Factual certainty 0.0-1.0.
    pub confidence: f32,

    // -- Content --
    /// The serialized payload bytes.
    pub content: Vec<u8>,
    /// Pre-computed embedding vector for semantic search.
    pub embedding: Option<Vec<f32>>,

    // -- Metadata --
    /// Freeform tags for categorization.
    pub tags: Vec<String>,
    /// Typed relationships to other memory objects.
    pub relationships: Vec<Relationship>,
    /// Extensible key-value metadata.
    pub metadata: HashMap<String, String>,

    // -- Lifecycle --
    /// Optional TTL-based expiration timestamp.
    pub expiration: Option<Timestamp>,
    /// SHA-256 checksum of the content field.
    pub checksum: [u8; 32],
}

impl MemoryObject {
    /// Create a new `MemoryObjectBuilder` for constructing a MemoryObject.
    pub fn builder() -> MemoryObjectBuilder {
        MemoryObjectBuilder::new()
    }
}

/// Builder for constructing `MemoryObject` instances.
///
/// Provides sensible defaults for all fields.
/// Only `content_type` and `content` are required to be set explicitly.
#[derive(Debug)]
pub struct MemoryObjectBuilder {
    id: Option<MemoryId>,
    version: Version,
    timestamp: Option<Timestamp>,
    created_by: String,
    source: MemorySource,
    memory_type: MemoryType,
    tier: MemoryTier,
    content_type: String,
    priority: u8,
    importance: f32,
    confidence: f32,
    content: Vec<u8>,
    embedding: Option<Vec<f32>>,
    tags: Vec<String>,
    relationships: Vec<Relationship>,
    metadata: HashMap<String, String>,
    expiration: Option<Timestamp>,
}

impl Default for MemoryObjectBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryObjectBuilder {
    /// Create a new builder with default values.
    pub fn new() -> Self {
        Self {
            id: None,
            version: Version::INITIAL,
            timestamp: None,
            created_by: "system".into(),
            source: MemorySource::System,
            memory_type: MemoryType::Working,
            tier: MemoryTier::Working,
            content_type: String::new(),
            priority: MemoryPriority::NORMAL.get(),
            importance: MemoryImportance::DEFAULT.get(),
            confidence: 1.0,
            content: Vec::new(),
            embedding: None,
            tags: Vec::new(),
            relationships: Vec::new(),
            metadata: HashMap::new(),
            expiration: None,
        }
    }

    /// Set the object ID. Auto-generated if not set.
    pub fn id(mut self, id: MemoryId) -> Self {
        self.id = Some(id);
        self
    }

    /// Set the version.
    pub fn version(mut self, version: Version) -> Self {
        self.version = version;
        self
    }

    /// Set the timestamp. Uses current time if not set.
    pub fn timestamp(mut self, timestamp: Timestamp) -> Self {
        self.timestamp = Some(timestamp);
        self
    }

    /// Set the creator identifier.
    pub fn created_by(mut self, created_by: impl Into<String>) -> Self {
        self.created_by = created_by.into();
        self
    }

    /// Set the source.
    pub fn source(mut self, source: MemorySource) -> Self {
        self.source = source;
        self
    }

    /// Set the memory type.
    pub fn memory_type(mut self, memory_type: MemoryType) -> Self {
        self.memory_type = memory_type;
        self
    }

    /// Set the storage tier.
    pub fn tier(mut self, tier: MemoryTier) -> Self {
        self.tier = tier;
        self
    }

    /// Set the content type (MIME-like).
    pub fn content_type(mut self, content_type: impl Into<String>) -> Self {
        self.content_type = content_type.into();
        self
    }

    /// Set the priority.
    pub fn priority(mut self, priority: MemoryPriority) -> Self {
        self.priority = priority.get();
        self
    }

    /// Set the importance.
    pub fn importance(mut self, importance: MemoryImportance) -> Self {
        self.importance = importance.get();
        self
    }

    /// Set the confidence.
    pub fn confidence(mut self, confidence: f32) -> Self {
        self.confidence = confidence;
        self
    }

    /// Set the content bytes.
    pub fn content(mut self, content: Vec<u8>) -> Self {
        self.content = content;
        self
    }

    /// Set the embedding vector.
    pub fn embedding(mut self, embedding: Option<Vec<f32>>) -> Self {
        self.embedding = embedding;
        self
    }

    /// Set the tags.
    pub fn tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    /// Add a single tag.
    pub fn tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Set the relationships.
    pub fn relationships(mut self, relationships: Vec<Relationship>) -> Self {
        self.relationships = relationships;
        self
    }

    /// Set the metadata map.
    pub fn metadata(mut self, metadata: HashMap<String, String>) -> Self {
        self.metadata = metadata;
        self
    }

    /// Insert a single metadata key-value pair.
    pub fn with_meta(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Set the expiration.
    pub fn expiration(mut self, expiration: Option<Timestamp>) -> Self {
        self.expiration = expiration;
        self
    }

    /// Build the `MemoryObject`.
    ///
    /// Auto-generates the ID, timestamp, and checksum if not explicitly set.
    pub fn build(self) -> MemoryObject {
        let id = self.id.unwrap_or_else(MemoryId::new);
        let timestamp = self.timestamp.unwrap_or_else(Timestamp::now);
        let checksum = Checksum::compute(&self.content);

        MemoryObject {
            id,
            version: self.version,
            timestamp,
            created_by: self.created_by,
            source: self.source,
            memory_type: self.memory_type,
            tier: self.tier,
            content_type: self.content_type,
            priority: self.priority,
            importance: self.importance,
            confidence: self.confidence,
            content: self.content,
            embedding: self.embedding,
            tags: self.tags,
            relationships: self.relationships,
            metadata: self.metadata,
            expiration: self.expiration,
            checksum: checksum.into_bytes(),
        }
    }
}

impl Checksum {
    fn into_bytes(self) -> [u8; 32] {
        *self.as_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_defaults() {
        let obj = MemoryObject::builder()
            .content_type("text/plain")
            .content(b"hello".to_vec())
            .build();

        assert_eq!(obj.version, Version::INITIAL);
        assert_eq!(obj.created_by, "system");
        assert_eq!(obj.source, MemorySource::System);
        assert_eq!(obj.memory_type, MemoryType::Working);
        assert_eq!(obj.tier, MemoryTier::Working);
        assert_eq!(obj.content_type, "text/plain");
        assert_eq!(obj.importance, 0.5);
        assert_eq!(obj.confidence, 1.0);
    }

    #[test]
    fn test_builder_custom_fields() {
        let id = MemoryId::new();
        let rel = Relationship::reference(id);
        let obj = MemoryObject::builder()
            .id(id)
            .version(Version::new(3))
            .created_by("agent-1")
            .source(MemorySource::Agent)
            .memory_type(MemoryType::Episodic)
            .tier(MemoryTier::Episodic)
            .content_type("application/json")
            .priority(MemoryPriority::HIGH)
            .importance(MemoryImportance::new(0.9).unwrap())
            .confidence(0.75)
            .content(br#"{"event":"login"}"#.to_vec())
            .embedding(Some(vec![0.1, 0.2, 0.3]))
            .tag("auth")
            .tag("security")
            .relationships(vec![rel])
            .with_meta("source_ip", "192.168.1.1")
            .expiration(Some(Timestamp::from_secs(1_000_000_000)))
            .build();

        assert_eq!(obj.id, id);
        assert_eq!(obj.version.get(), 3);
        assert_eq!(obj.created_by, "agent-1");
        assert_eq!(obj.source, MemorySource::Agent);
        assert_eq!(obj.memory_type, MemoryType::Episodic);
        assert_eq!(obj.tier, MemoryTier::Episodic);
        assert_eq!(obj.priority, 192);
        assert!((obj.importance - 0.9).abs() < f32::EPSILON);
        assert!((obj.confidence - 0.75).abs() < f32::EPSILON);
        assert_eq!(obj.tags.len(), 2);
        assert_eq!(obj.relationships.len(), 1);
        assert_eq!(obj.metadata.get("source_ip").unwrap(), "192.168.1.1");
        assert!(obj.expiration.is_some());
    }

    #[test]
    fn test_auto_id_generation() {
        let a = MemoryObject::builder()
            .content_type("text")
            .content(vec![1])
            .build();
        let b = MemoryObject::builder()
            .content_type("text")
            .content(vec![2])
            .build();
        assert_ne!(a.id, b.id);
    }

    #[test]
    fn test_auto_timestamp() {
        let obj = MemoryObject::builder()
            .content_type("text")
            .content(vec![])
            .build();
        let now = Timestamp::now();
        let diff = now.as_nanos() - obj.timestamp.as_nanos();
        assert!(diff.abs() < 1_000_000_000); // within 1 second
    }

    #[test]
    fn test_auto_checksum() {
        let obj = MemoryObject::builder()
            .content_type("text")
            .content(b"hello world".to_vec())
            .build();
        let expected = Checksum::compute(b"hello world");
        assert_eq!(obj.checksum, *expected.as_bytes());
    }

    #[test]
    fn test_serialization_roundtrip() {
        let obj = MemoryObject::builder()
            .content_type("application/json")
            .content(br#"{"key":"value"}"#.to_vec())
            .tag("test")
            .build();

        let json = serde_json::to_string(&obj).unwrap();
        let restored: MemoryObject = serde_json::from_str(&json).unwrap();
        assert_eq!(obj.id, restored.id);
        assert_eq!(obj.version, restored.version);
        assert_eq!(obj.content_type, restored.content_type);
        assert_eq!(obj.tags, restored.tags);
        assert_eq!(obj.checksum, restored.checksum);
    }
}
