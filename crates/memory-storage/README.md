# memory-storage

Backend-agnostic storage abstraction for the AI-native OS Memory Platform.

## Traits

- **`MemoryStore`** — High-level CRUD, batch, query, and management operations on `MemoryObject` records.
- **`Transaction`** — Begin, commit, and rollback transactional memory operations.
- **`StorageBackend`** — Low-level key-value storage abstraction for pluggable database backends.
- **`Query`** — Filtered query execution against a memory store.
- **`BatchOperation`** — Efficient batch insert, get, and delete operations.

## Implementations

- **`InMemoryStore`** — Thread-safe in-memory store backed by `RwLock<HashMap>`. Suitable for testing and single-node deployments.
- **`MockStore`** — Configurable mock for testing higher-level memory subsystems. Records all operations for assertion.

## Design

The traits form a hierarchy from low-level (`StorageBackend`) to high-level (`MemoryStore`). The `InMemoryStore` and `MockStore` implement `MemoryStore` directly. Future backends (SQLite, PostgreSQL, RocksDB) implement `StorageBackend` and are wrapped by a `BackedStore` that implements `MemoryStore`.
