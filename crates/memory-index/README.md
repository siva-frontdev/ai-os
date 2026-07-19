# memory-index

Index trait definitions for the AI-native OS Memory Platform.

## Traits

- **`MetadataIndex`** — Index and search memory objects by metadata key-value pairs.
- **`TagIndex`** — Index and search memory objects by tags.
- **`RelationshipIndex`** — Index and search memory objects by relationships (graph adjacency).
- **`TimeIndex`** — Index and search memory objects by time range.

Each trait provides a `Default*` no-op implementation that returns empty results. Production implementations (in-memory HashMap-based, SQL-backed, etc.) are provided by future crates.

## Architecture

These index traits complement the `MemoryIndex` trait (embedding-based vector search, defined in a future stage) to form a complete search infrastructure:

- `MetadataIndex` — find objects by metadata key-value pairs
- `TagIndex` — find objects by tags
- `RelationshipIndex` — graph adjacency queries
- `TimeIndex` — temporal range queries
- `MemoryIndex` — embedding similarity search (future)
