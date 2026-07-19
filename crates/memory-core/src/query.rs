use std::collections::HashMap;
use std::ops::Range;
use serde::{Deserialize, Serialize};

use crate::types::{MemorySource, MemoryTier, MemoryType, Timestamp};

/// Specification for querying memory objects by filter criteria.
///
/// All filter fields are optional — omitted fields are not filtered.
/// Use `QueryFilter::default()` for an unfiltered query that returns
/// all objects up to the default limit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryFilter {
    /// Filter by storage tier.
    pub tier: Option<MemoryTier>,
    /// Filter by cognitive memory type.
    pub memory_type: Option<MemoryType>,
    /// Filter by tags (objects matching any of the specified tags).
    pub tags: Option<Vec<String>>,
    /// Filter by origin source.
    pub source: Option<MemorySource>,
    /// Filter by creator identifier.
    pub created_by: Option<String>,
    /// Filter by time range (inclusive).
    pub time_range: Option<Range<Timestamp>>,
    /// Minimum priority threshold.
    pub priority_min: Option<u8>,
    /// Minimum importance threshold.
    pub importance_min: Option<f32>,
    /// Minimum confidence threshold.
    pub confidence_min: Option<f32>,
    /// Maximum number of results to return.
    pub limit: usize,
    /// Number of results to skip (for pagination).
    pub offset: usize,
    /// Field to sort results by.
    pub sort_by: SortField,
    /// Sort direction.
    pub sort_order: SortOrder,
}

impl Default for QueryFilter {
    fn default() -> Self {
        Self {
            tier: None,
            memory_type: None,
            tags: None,
            source: None,
            created_by: None,
            time_range: None,
            priority_min: None,
            importance_min: None,
            confidence_min: None,
            limit: 100,
            offset: 0,
            sort_by: SortField::Timestamp,
            sort_order: SortOrder::Descending,
        }
    }
}

/// Fields that query results can be sorted by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SortField {
    /// Sort by creation/modification timestamp.
    Timestamp,
    /// Sort by priority value.
    Priority,
    /// Sort by importance score.
    Importance,
    /// Sort by confidence score.
    Confidence,
    /// Sort by version number.
    Version,
    /// Sort by access count (for stores that track this).
    AccessCount,
}

impl Default for SortField {
    fn default() -> Self {
        Self::Timestamp
    }
}

/// Sort direction for query results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SortOrder {
    /// Ascending (lowest first).
    Ascending,
    /// Descending (highest first).
    Descending,
}

impl Default for SortOrder {
    fn default() -> Self {
        Self::Descending
    }
}

/// Statistics about a memory store's current state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageStats {
    /// Total number of stored objects.
    pub total_objects: u64,
    /// Total bytes consumed by stored objects.
    pub total_bytes: u64,
    /// Count of objects per storage tier.
    pub tier_counts: HashMap<MemoryTier, u64>,
    /// Count of objects per cognitive memory type.
    pub type_counts: HashMap<MemoryType, u64>,
    /// Average object size in bytes.
    pub average_object_size: f64,
}

impl StorageStats {
    /// Create a new empty StorageStats.
    pub fn new() -> Self {
        Self {
            total_objects: 0,
            total_bytes: 0,
            tier_counts: HashMap::new(),
            type_counts: HashMap::new(),
            average_object_size: 0.0,
        }
    }
}

impl Default for StorageStats {
    fn default() -> Self {
        Self::new()
    }
}

/// Capacity information for a bounded memory tier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapacityInfo {
    /// Maximum number of entries allowed.
    pub max_entries: usize,
    /// Current number of entries.
    pub current_entries: usize,
    /// Maximum bytes allowed.
    pub max_bytes: u64,
    /// Current bytes used.
    pub current_bytes: u64,
}

impl CapacityInfo {
    /// Create new CapacityInfo.
    pub fn new(max_entries: usize, current_entries: usize, max_bytes: u64, current_bytes: u64) -> Self {
        Self { max_entries, current_entries, max_bytes, current_bytes }
    }

    /// Return the usage percentage by entry count (0.0 - 100.0).
    pub fn entry_usage_pct(&self) -> f64 {
        if self.max_entries == 0 {
            return 0.0;
        }
        (self.current_entries as f64 / self.max_entries as f64) * 100.0
    }

    /// Return the usage percentage by bytes (0.0 - 100.0).
    pub fn byte_usage_pct(&self) -> f64 {
        if self.max_bytes == 0 {
            return 0.0;
        }
        (self.current_bytes as f64 / self.max_bytes as f64) * 100.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_filter_defaults() {
        let filter = QueryFilter::default();
        assert_eq!(filter.limit, 100);
        assert_eq!(filter.offset, 0);
        assert_eq!(filter.sort_by, SortField::Timestamp);
        assert_eq!(filter.sort_order, SortOrder::Descending);
        assert!(filter.tier.is_none());
    }

    #[test]
    fn test_query_filter_custom() {
        let filter = QueryFilter {
            tier: Some(MemoryTier::Working),
            memory_type: Some(MemoryType::Episodic),
            tags: Some(vec!["important".into()]),
            limit: 50,
            sort_by: SortField::Importance,
            sort_order: SortOrder::Ascending,
            ..Default::default()
        };
        assert_eq!(filter.tier, Some(MemoryTier::Working));
        assert_eq!(filter.limit, 50);
    }

    #[test]
    fn test_storage_stats_new() {
        let stats = StorageStats::new();
        assert_eq!(stats.total_objects, 0);
        assert_eq!(stats.total_bytes, 0);
    }

    #[test]
    fn test_capacity_info() {
        let info = CapacityInfo::new(100, 50, 1024, 512);
        assert!((info.entry_usage_pct() - 50.0).abs() < f64::EPSILON);
        assert!((info.byte_usage_pct() - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_capacity_info_zero_max() {
        let info = CapacityInfo::new(0, 0, 0, 0);
        assert!((info.entry_usage_pct() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_storage_stats_tier_counts() {
        let mut stats = StorageStats::new();
        stats.tier_counts.insert(MemoryTier::Working, 10);
        stats.tier_counts.insert(MemoryTier::Episodic, 5);
        assert_eq!(stats.tier_counts.get(&MemoryTier::Working), Some(&10));
        assert_eq!(stats.tier_counts.get(&MemoryTier::Episodic), Some(&5));
    }
}
