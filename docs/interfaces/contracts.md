# Cross-Cutting Interface Contracts

This document defines the contracts that every interface in every layer must satisfy. These contracts are enforced by convention, code review, and (where possible) by `clippy` lints and custom `cfg`-level assertions.

---

## Trait Contract

Every public trait in the OS interface surface **must** satisfy:

```rust
pub trait SomeTrait: Debug + Send + Sync {
    // methods here
}
```

- `Debug` — required for structured logging and observability.
- `Send` — traits must be ownership-transferable across thread boundaries.
- `Sync` — traits must be safely shareable by reference (behind `&self`).

All trait methods **must** take `&self` (never `&mut self`). Mutation is handled via interior mutability (e.g., `Arc<RwLock<>>`, `Atomic*`).

**Generic methods are forbidden.** Every method must use concrete types or `dyn Trait` objects. This ensures all traits are `dyn`-compatible and can be stored as `Arc<dyn Trait>`.

✅ Allowed:
```rust
async fn publish(&self, event: Box<dyn Event>) -> Result<EventId, BusError>;
```

❌ Forbidden:
```rust
async fn publish<T: Event>(&self, event: T) -> Result<EventId, BusError>;
```

Associated types are preferred over generics when type variation is needed:
```rust
pub trait EventBus: Debug + Send + Sync {
    type Event: Event + 'static;
    async fn publish(&self, event: Self::Event) -> Result<EventId, BusError>;
}
```

---

## Async Contract

- All I/O-bound public methods **must** be `async fn`.
- CPU-bound operations (crypto, sorting, model inference, serialization) **must not** block the async runtime. Use `tokio::task::spawn_blocking` for these.
- Async methods **must** be cancellation-safe. No internal state may be corrupted if the caller drops the future mid-flight.
- Async trait methods use `#[async_trait]` from the `async-trait` crate. The macro generates `Box<dyn Future>` return types — this is acceptable for the interface boundary.
- Lock acquisitions inside async methods must use `std::sync::Mutex` (not `tokio::sync::Mutex`) unless the lock is held across `.await` points.
- Channels between layers must be bounded. Unbounded channels are only permitted for internal, trusted communication.

---

## Error Contract

- Every public operation **must** return `Result<T, E>` where `E` is a layer-specific error enum.
- **No panics in release mode.** Panics in debug mode are permitted only for programmer errors (e.g., assertion failures, unreachable!).
- Error enums **must** implement `Debug + Display + Send + Sync + Error`.
- Each error variant **must** have a doc comment explaining when it occurs.
- Error variants **must** be specific. `Io(io::Error)` is acceptable only when wrapped with context. `Unknown` variants are forbidden.
- Error recovery strategies **must** be documented in the variant doc comment (e.g., "Retry with exponential backoff" / "Fatal, service should stop").
- Layers **must not** leak their dependency errors. A [OSAL](osal.md) `FsError` must not appear in a [Brain](brain.md) trait signature. Map dependency errors into layer-specific errors.

---

## Event Contract

- All events are **immutable** after publication. Once an event is dispatched on the `EventBus`, no code may mutate its payload.
- Every event **must** carry a `TraceId` for distributed tracing across layers.
- Event routing uses `TypeId` — subscribers register for a concrete type and receive only matching events. No string-based topic matching at the bus layer.
- Event payloads **must** implement `Event`:
  ```rust
  pub trait Event: Debug + Send + Sync + 'static {
      fn trace_id(&self) -> TraceId;
      fn event_name(&self) -> &'static str;
  }
  ```
- Events are **best-effort delivery**. The `EventBus` does not guarantee delivery to all subscribers if the channel is full.
- Events **must not** contain sensitive data (passwords, tokens, keys). If sensitive data must be carried, use a reference handle.
- Backpressure is handled at the channel boundary. If a subscriber is slow, the channel drops the oldest event (sliding window) or blocks the publisher (bounded channel with backpressure).

---

## Capability Contract

Every entry point that accesses OS resources **must** accept a `CapabilityContext` parameter:

```rust
pub struct CapabilityContext {
    session_id: SessionId,
    capabilities: CapabilitySet,
    trace_id: TraceId,
}
```

- Capabilities are checked **before** the operation begins.
- If the context lacks the required capability, the operation returns `PermissionError::Denied` immediately.
- Capability checks are **not** cached across calls — re-validation is required per invocation.
- `CapabilityContext` is propagated through call stacks. Lower layers never check capabilities themselves; they trust the caller.
- Capabilities are hierarchical: `MemoryWrite` implies `MemoryRead` for the same scope.
- Operations that require escalation (e.g., acquiring a new capability) go through a dedicated `CapabilityManager` in [Runtime](runtime.md).

```rust
// Every public method that touches OS state:
pub trait FileSystem: Debug + Send + Sync {
    async fn open(
        &self,
        ctx: &CapabilityContext,   // <-- required
        path: &Path,
        opts: OpenOpts,
    ) -> Result<FileHandle, FsError>;
}
```

---

## Documentation Contract

- Every public item (trait, struct, enum, method, function, const, type alias) **must** have a doc comment (`///`).
- Every public trait **must** have a module-level doc comment explaining its purpose, role in the layer stack, and dependencies.
- Every method **must** document:
  - What it does (one sentence minimum)
  - Parameters (if non-obvious)
  - Return value
  - Error variants that callers should handle
  - Cancellation safety if async
- Doc examples are encouraged but not required.
- Use `# Errors` in doc comments to document error conditions per the Rust API guidelines.
- Cross-reference related interfaces using relative links: `[EventBus](core.md)`, `[KernelFacade](osal.md)`.

---

## Thread Safety Contract

- All public types **must** implement `Send + Sync`.
- Shared mutable state **must** be behind `Arc<`lock type`>`.
- Lock scopes **must** be as short as possible. Never hold a lock across an `.await` point (unless using `tokio::sync::Mutex` and the held data must be consistent across the await).
- Prefer `Arc<RwLock<T>>` for read-mostly data. Prefer `Atomic*` for counters and flags. Prefer `Arc<Mutex<T>>` for write-heavy data.
- `unsafe` code is forbidden in interface trait definitions. `unsafe` in implementations must be justified by a `// SAFETY:` comment.
- Structs that hold `Arc<dyn Trait>` fields are automatically `Send + Sync` if the trait is `Send + Sync`.
- Lock ordering must be documented per module to avoid deadlocks. Lock hierarchies are enforced by convention.

Lock hierarchy (lowest to highest):
1. `ResourceManager` counters
2. `Scheduler` task queue
3. `MemoryStore` / `MemoryIndex`
4. `KnowledgeGraph`
5. `Config` store

Acquiring a lower-numbered lock while holding a higher-numbered lock is forbidden.

---

## Versioning Contract

- The OS platform API follows **semantic versioning** (`major.minor.patch`).
- Breaking changes (trait method removed, signature change, behavior change) **only** in major version bumps.
- New traits, new methods on existing traits, and new error variants are minor version bumps.
- Internal implementation changes (no public API impact) are patch version bumps.
- Deprecated APIs **must** remain for at least one full minor release cycle before removal.
- Deprecated items **must** use `#[deprecated(note = "use ... instead")]`.
- The deprecation notice **must** specify the minimum version in which the item will be removed.
- All layers are versioned as a single unit. Individual layer versioning is not supported.
- The `CHANGELOG.md` is updated per the Keep a Changelog format for every release.

---

## Portability Contract

- **No OS-specific types in trait signatures.** This is the cardinal rule. Common trap types to avoid:
  - ❌ `std::os::unix::io::RawFd` → use `FileHandle` (defined in OSAL)
  - ❌ `std::os::unix::process::ExitStatusExt` → use `ExitStatus` (defined in OSAL)
  - ❌ `nix::sys::signal::Signal` → use `Signal` enum (defined in OSAL)
  - ❌ `iovec`, `msghdr`, `termios`, `stat` → use OSAL wrapper types
  - ❌ `DWORD`, `HANDLE`, `LPCWSTR` → mapped to idiomatic Rust types
- **All OS-specific code goes behind [OSAL](osal.md).** If a subsystem needs `open(2)`, it calls `KernelFacade::filesystem().open(...)` — never `libc::open`.
- No `#[cfg(target_os = "...")]` in public API surfaces. Conditional compilation is restricted to implementation modules.
- Path handling uses `Path` / `PathBuf` from `std` — never C strings or OS-path reprs.
- Error types from OS-specific syscalls are wrapped and mapped before reaching the public API.
- Time types use `std::time::Duration` / `Instant`. No `libc::timespec` or `clock_gettime` in signatures.
- Feature-gated code (e.g., `mem-rocksdb`) compiles on all tier-1 targets, even if the backend is non-functional (returns `Unsupported`).
