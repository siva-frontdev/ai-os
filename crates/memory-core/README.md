# memory-core

Foundation crate for the AI-native OS Memory Platform. Defines all shared types, traits, errors, events, and validation that other memory crates depend on.

## Types

- `MemoryId` — UUID v7 identifier
- `MemoryObject` — universal memory record with versioning, checksums, and metadata
- `MemorySource` — origin classification (User, System, Agent, Derived, External)
- `MemoryTier` — storage tier (Working, Episodic, Semantic, Archive)
- `MemoryType` — cognitive type (Working, Episodic, Semantic, Knowledge)
- `MemoryPriority` — 0-255 priority value
- `MemoryImportance` — 0.0-1.0 importance score
- `Version` — monotonic version counter
- `Checksum` — SHA-256 content checksum
- `Metadata` — extensible key-value metadata map
- `MemoryRelationship` — typed link between memory objects
- `RelationType` — relationship classifications
- `QueryFilter` — filtered query specification
- `SortField` / `SortOrder` — sort configuration
- `StorageStats` / `CapacityInfo` — storage statistics

## Validation

The `Validate` trait provides bounds checking for importance, priority, weight, and content integrity.

## Serialization

Serde-based serialization helpers for all types.

## Events

`MemoryEvent` enum covers all memory subsystem events (stored, recalled, updated, deleted, consolidated, pruned, etc.).
