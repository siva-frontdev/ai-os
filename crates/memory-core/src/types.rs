use std::fmt;
use std::str::FromStr;
use serde::{Deserialize, Serialize};

/// A unique identifier for a memory object.
///
/// Uses UUID v7 (time-sortable) to enable chronological ordering
/// without a separate timestamp index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MemoryId(uuid::Uuid);

impl MemoryId {
    /// Create a new random MemoryId (UUID v7).
    pub fn new() -> Self {
        Self(uuid::Uuid::now_v7())
    }

    /// Create a MemoryId from a raw UUID.
    pub const fn from_uuid(uuid: uuid::Uuid) -> Self {
        Self(uuid)
    }

    /// Return the inner UUID.
    pub const fn as_uuid(&self) -> &uuid::Uuid {
        &self.0
    }

    /// Consume and return the inner UUID.
    pub fn into_uuid(self) -> uuid::Uuid {
        self.0
    }
}

impl Default for MemoryId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for MemoryId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for MemoryId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        uuid::Uuid::from_str(s).map(Self)
    }
}

impl From<uuid::Uuid> for MemoryId {
    fn from(uuid: uuid::Uuid) -> Self {
        Self(uuid)
    }
}

/// UNIX epoch timestamp in nanoseconds.
///
/// Uses `i64` to cover 292 years of nanosecond precision
/// (from 1970 to ~2262).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Timestamp(i64);

impl Timestamp {
    /// The minimum representable timestamp.
    pub const MIN: Self = Self(i64::MIN);
    /// The maximum representable timestamp.
    pub const MAX: Self = Self(i64::MAX);

    /// Create a new timestamp from nanoseconds since UNIX epoch.
    pub const fn from_nanos(nanos: i64) -> Self {
        Self(nanos)
    }

    /// Return the raw nanosecond value.
    pub const fn as_nanos(&self) -> i64 {
        self.0
    }

    /// Return the timestamp in seconds (truncated).
    pub fn as_secs(&self) -> i64 {
        self.0 / 1_000_000_000
    }

    /// Create a timestamp from seconds since UNIX epoch.
    ///
    /// Returns `Timestamp::MAX` if the result would overflow.
    pub const fn from_secs(secs: i64) -> Self {
        match secs.checked_mul(1_000_000_000) {
            Some(nanos) => Self(nanos),
            None => Self::MAX,
        }
    }

    /// Return the current wall-clock time as a Timestamp.
    pub fn now() -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock before epoch");
        Self(now.as_nanos() as i64)
    }
}

impl Default for Timestamp {
    fn default() -> Self {
        Self::now()
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// The origin source of a memory object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MemorySource {
    /// Created directly by a human user.
    User,
    /// Created by the operating system platform.
    System,
    /// Created by an AI agent.
    Agent,
    /// Derived or inferred from other memory objects.
    Derived,
    /// Imported from an external system.
    External,
}

impl MemorySource {
    /// All variants of MemorySource.
    pub const ALL: &'static [Self] = &[Self::User, Self::System, Self::Agent, Self::Derived, Self::External];
}

/// The physical storage tier of a memory object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MemoryTier {
    /// Volatile, in-memory, sub-microsecond access.
    Working,
    /// Fast persistent storage (e.g. SQLite).
    Episodic,
    /// Durable persistent storage (e.g. SQLite / PostgreSQL).
    Semantic,
    /// Cold storage (compressed or object store).
    Archive,
}

impl fmt::Display for MemoryTier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Working => write!(f, "Working"),
            Self::Episodic => write!(f, "Episodic"),
            Self::Semantic => write!(f, "Semantic"),
            Self::Archive => write!(f, "Archive"),
        }
    }
}

impl MemoryTier {
    /// All variants of MemoryTier.
    pub const ALL: &'static [Self] = &[Self::Working, Self::Episodic, Self::Semantic, Self::Archive];
}

/// The cognitive type classification of a memory object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MemoryType {
    /// Volatile, bounded, sub-microsecond working memory.
    Working,
    /// Temporal event sequences with context.
    Episodic,
    /// Generalized facts, concepts, abstractions.
    Semantic,
    /// Structured entities and relations (knowledge graph).
    Knowledge,
}

impl MemoryType {
    /// All variants of MemoryType.
    pub const ALL: &'static [Self] = &[Self::Working, Self::Episodic, Self::Semantic, Self::Knowledge];
}

/// The type of relationship between two memory objects.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RelationType {
    /// Object B is derived from object A.
    DerivesFrom,
    /// Object A references object B.
    References,
    /// Object B is a part of object A.
    PartOf,
    /// Object B follows object A in a temporal sequence.
    Sequence,
    /// Object A contradicts object B.
    Contradicts,
    /// Object A supports or confirms object B.
    Supports,
    /// Extension point for custom relation types.
    Custom(String),
}

/// A priority value for memory objects (0-255).
///
/// Higher values indicate higher priority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct MemoryPriority(u8);

impl MemoryPriority {
    /// Minimum priority value.
    pub const MIN: Self = Self(0);
    /// Maximum priority value.
    pub const MAX: Self = Self(255);
    /// Default / normal priority.
    pub const NORMAL: Self = Self(128);
    /// High priority.
    pub const HIGH: Self = Self(192);
    /// Low priority.
    pub const LOW: Self = Self(64);

    /// Create a new MemoryPriority. Any u8 is valid (0-255).
    pub const fn new(value: u8) -> Self {
        Self(value)
    }

    /// Return the raw u8 value.
    pub const fn get(&self) -> u8 {
        self.0
    }
}

impl Default for MemoryPriority {
    fn default() -> Self {
        Self::NORMAL
    }
}

impl From<u8> for MemoryPriority {
    fn from(value: u8) -> Self {
        Self(value)
    }
}

/// An importance score for memory objects (0.0 - 1.0).
///
/// Higher values indicate greater importance. Computed by the
/// learning engine based on access frequency, recency, and explicit priority.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct MemoryImportance(f32);

impl MemoryImportance {
    /// Minimum importance value.
    pub const MIN: Self = Self(0.0);
    /// Maximum importance value.
    pub const MAX: Self = Self(1.0);
    /// Default importance.
    pub const DEFAULT: Self = Self(0.5);

    /// Create a new MemoryImportance. Returns an error if value is outside [0.0, 1.0].
    pub fn new(value: f32) -> Result<Self, super::MemoryError> {
        if !(0.0..=1.0).contains(&value) {
            return Err(super::MemoryError::ValidationError(format!(
                "importance must be between 0.0 and 1.0, got {}",
                value
            )));
        }
        Ok(Self(value))
    }

    /// Create a new MemoryImportance without validation. Only for internal use.
    #[allow(dead_code)]
    pub(crate) const fn new_unchecked(value: f32) -> Self {
        Self(value)
    }

    /// Return the raw f32 value.
    pub const fn get(&self) -> f32 {
        self.0
    }
}

impl Default for MemoryImportance {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// A monotonic version counter for memory objects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Version(u64);

impl Version {
    /// The initial version of any new memory object.
    pub const INITIAL: Self = Self(1);

    /// Create a new version.
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Return the raw u64 value.
    pub const fn get(&self) -> u64 {
        self.0
    }

    /// Increment the version, producing the next value.
    pub fn next(&self) -> Self {
        Self(self.0.wrapping_add(1))
    }
}

impl Default for Version {
    fn default() -> Self {
        Self::INITIAL
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A SHA-256 checksum for verifying memory object content integrity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Checksum([u8; 32]);

impl Checksum {
    /// Create a checksum from a byte array.
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Compute the SHA-256 checksum of a byte slice.
    pub fn compute(data: &[u8]) -> Self {
        use sha2::Digest;
        let hash = sha2::Sha256::digest(data);
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&hash);
        Self(arr)
    }

    /// Return a reference to the inner byte array.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Verify that a byte slice matches this checksum.
    pub fn verify(&self, data: &[u8]) -> bool {
        *self == Self::compute(data)
    }
}

impl fmt::Display for Checksum {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in &self.0 {
            write!(f, "{:02x}", byte)?;
        }
        Ok(())
    }
}

/// Extensible key-value metadata for memory objects.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Metadata(std::collections::HashMap<String, String>);

impl Metadata {
    /// Create an empty metadata map.
    pub fn new() -> Self {
        Self(std::collections::HashMap::new())
    }

    /// Insert a key-value pair.
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) -> Option<String> {
        self.0.insert(key.into(), value.into())
    }

    /// Get a value by key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).map(|s| s.as_str())
    }

    /// Remove a key-value pair.
    pub fn remove(&mut self, key: &str) -> Option<String> {
        self.0.remove(key)
    }

    /// Return true if the metadata is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Return the number of entries.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Return an iterator over key-value pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.0.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    /// Consume and return the inner HashMap.
    pub fn into_inner(self) -> std::collections::HashMap<String, String> {
        self.0
    }
}

impl Default for Metadata {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Into<String>, V: Into<String>> FromIterator<(K, V)> for Metadata {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        Self(iter.into_iter().map(|(k, v)| (k.into(), v.into())).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_id_creation() {
        let id = MemoryId::new();
        assert_ne!(id, MemoryId::default());
        assert!(id.as_uuid().get_version() == Some(uuid::Version::SortRand));
    }

    #[test]
    fn test_timestamp_now() {
        let ts = Timestamp::now();
        assert!(ts.as_nanos() > 0);
        assert_eq!(ts.as_secs(), ts.as_nanos() / 1_000_000_000);
    }

    #[test]
    fn test_memory_priority() {
        assert_eq!(MemoryPriority::new(0), MemoryPriority::MIN);
        assert_eq!(MemoryPriority::new(255), MemoryPriority::MAX);
        assert_eq!(MemoryPriority::default(), MemoryPriority::NORMAL);
        assert!(MemoryPriority::HIGH > MemoryPriority::LOW);
    }

    #[test]
    fn test_memory_importance() {
        assert!(MemoryImportance::new(0.0).is_ok());
        assert!(MemoryImportance::new(0.5).is_ok());
        assert!(MemoryImportance::new(1.0).is_ok());
        assert!(MemoryImportance::new(1.5).is_err());
        assert!(MemoryImportance::new(-0.1).is_err());
    }

    #[test]
    fn test_version() {
        assert_eq!(Version::default(), Version::INITIAL);
        assert_eq!(Version::new(1).next(), Version::new(2));
    }

    #[test]
    fn test_checksum() {
        let data = b"hello world";
        let cs = Checksum::compute(data);
        assert!(cs.verify(data));
        assert!(!cs.verify(b"wrong data"));
    }

    #[test]
    fn test_metadata() {
        let mut meta = Metadata::new();
        assert!(meta.is_empty());
        meta.insert("key1", "value1");
        assert_eq!(meta.get("key1"), Some("value1"));
        assert_eq!(meta.len(), 1);
        let removed = meta.remove("key1");
        assert_eq!(removed, Some("value1".into()));
        assert!(meta.is_empty());
    }

    #[test]
    fn test_memory_source_all() {
        assert_eq!(MemorySource::ALL.len(), 5);
    }

    #[test]
    fn test_memory_tier_all() {
        assert_eq!(MemoryTier::ALL.len(), 4);
    }

    #[test]
    fn test_memory_type_all() {
        assert_eq!(MemoryType::ALL.len(), 4);
    }

    #[test]
    fn test_memory_id_display() {
        let id = MemoryId::new();
        let s = id.to_string();
        assert_eq!(s.len(), 36);
        assert!(s.contains('-'));
    }

    #[test]
    fn test_memory_id_from_str() {
        let id = MemoryId::new();
        let s = id.to_string();
        let parsed: MemoryId = s.parse().unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn test_timestamp_from_secs() {
        let ts = Timestamp::from_secs(1_000_000_000);
        assert_eq!(ts.as_secs(), 1_000_000_000);
        assert_eq!(ts.as_nanos(), 1_000_000_000_000_000_000);
    }

    #[test]
    fn test_metadata_from_iterator() {
        let meta: Metadata = vec![("a", "1"), ("b", "2")].into_iter().collect();
        assert_eq!(meta.len(), 2);
        assert_eq!(meta.get("a"), Some("1"));
        assert_eq!(meta.get("b"), Some("2"));
    }
}
