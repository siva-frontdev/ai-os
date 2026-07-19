# Coding Standards

## Introduction

This document defines the Rust coding standards for the AI-native OS project. Every contribution must conform to these standards. Automated enforcement is applied through CI; manual review catches what automation cannot.

These standards are derived from the [design principles](principles.md), particularly Principle 7 (Explicit over Implicit) and Principle 12 (Documentation as Source of Truth).

---

## Naming Conventions

The project follows the Rust API Guidelines (RFC 430) with the following specifics:

### General Rules

| Category | Convention | Example |
|---|---|---|
| Types, traits, enums | `UpperCamelCase` | `EventBus`, `ServiceLifecycle` |
| Functions, methods | `snake_case` | `dispatch_event`, `check_permission` |
| Variables, fields | `snake_case` | `event_id`, `trace_context` |
| Constants, statics | `SCREAMING_SNAKE_CASE` | `MAX_RETRY_COUNT`, `DEFAULT_TIMEOUT_MS` |
| Modules | `snake_case` (prefer single word) | `scheduler`, `health` |
| Type parameters | concise `UpperCamelCase` | `T`, `E`, `Handler` |
| Lifetime parameters | single lowercase | `'a`, `'ctx` |
| Macro names | `snake_case` | `declare_event!`, `event_bus!` |
| Cargo crate names | `kebab-case` | `ai-os-core`, `ai-os-runtime` |
| Feature flags | `snake_case` | `unstable_features`, `metrics` |

### Acronyms

Treat acronyms as normal words in CamelCase: `HttpClient`, not `HTTPClient`. Treat acronyms as lowercase in snake_case: `http_client`, not `HTTP_Client`.

### Getter / Setter Conventions

- Getter methods omit `get_`: `fn config(&self) -> &Config`, not `fn get_config`.
- Setter methods use `set_`: `fn set_config(&mut self, config: Config)`.
- Boolean getters use `is_` or `has_`: `is_running()`, `has_permission()`.

---

## Formatting

All Rust code must be formatted with `rustfmt` using the project configuration. The project does not override any default `rustfmt` options.

```bash
# Format the entire workspace
cargo fmt --all

# Check formatting in CI
cargo fmt --all -- --check
```

Newlines at end of file are mandatory. Trailing whitespace is forbidden. Lines should not exceed 100 characters (the default `rustfmt` setting).

---

## Linting

All Rust code must pass `clippy` without warnings. The project uses the following clippy configuration in `.cargo/config.toml` or the workspace `Cargo.toml`:

```toml
[workspace.lints.clippy]
pedantic = "warn"
nursery = "allow"
cargo = "warn"
```

Run clippy on the workspace before every commit:

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

Specific clippy warnings may be suppressed only with a documented reason:

```rust
#[allow(clippy::cast_possible_truncation)]
// Required because we truncate u64 to u32 for the FFI boundary.
let truncated = value as u32;
```

---

## Error Handling

### Custom Error Types

Every crate defines a unified error enum using `thiserror`. The enum uses `#[derive(Debug, Error)]` and `#[error("...")]` attribute macros for display messages.

Core crate pattern (see `core/src/error.rs`):

```rust
#[derive(Debug, Error)]
pub enum CoreError {
    #[error("configuration key `{key}` not found")]
    ConfigNotFound { key: String },

    #[error("event handler for `{event_type}` failed: {detail}")]
    HandlerFailed { event_type: &'static str, detail: String },

    #[error("{0}")]
    General(String),
}
```

Runtime crate pattern (see `runtime/src/error.rs`):

```rust
#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("scheduler error: {0}")]
    Scheduler(String),

    #[error("permission denied: {0}")]
    PermissionDenied(String),

    #[error("core error: {0}")]
    Core(#[from] CoreError),

    #[error("{0}")]
    General(String),
}
```

### Error Design Guidelines

1. Each variant must have a unique, descriptive `#[error("...")]` message with display parameters.
2. Use `#[from]` to auto-convert dependency error types where the conversion is unambiguous.
3. Include structured fields (`{ key }`, `{ detail }`) rather than embedding them in a string.
4. Provide a `#[error("{0}")] General(String)` variant as a catch-all for one-off errors. Prefer specific variants when a pattern repeats.
5. Implement `pub` convenience constructors on the error type for commonly-created variants.

### Result Type

Each crate exposes a type alias:

```rust
// In core/src/lib.rs
pub type CoreResult<T> = Result<T, CoreError>;

// In runtime/src/lib.rs
pub type RuntimeResult<T> = Result<T, RuntimeError>;
```

Public functions that can fail must return `Result<T, E>` where `E` is the crate error type (or a specific variant). Use `Box<dyn Error>` only in binary entry points (main functions).

### Use of anyhow / thiserror

- `thiserror` is used in library crates for defining error types.
- `anyhow` is used in binary targets, integration tests, and examples where convenience is preferred over structured error types.
- Library crates must not depend on `anyhow` in their public API.

### Panic Policy

`panic!`, `unwrap()`, `expect()` are forbidden in production code. Use error propagation (`?`, `map_err`, `context`) instead.

Allowed exceptions:
- Testing code (where `unwrap()` is acceptable).
- `expect()` with a meaningful message when the error indicates a programming invariant violation (e.g., in `From` implementations).
- Unreachable code paths: `unreachable!()` only when the compiler cannot prove a match is exhaustive.

---

## Unsafe Code Policy

**Unsafe code is banned** except in explicitly reviewed FFI bindings.

### FFI Exceptions

1. Every `unsafe` block must be accompanied by a `// SAFETY:` comment explaining the invariants that justify the block.
2. FFI functions must be wrapped in safe abstractions. The abstraction is responsible for upholding safety invariants.
3. External C libraries must be vendored or pinned to a specific version, with security patches tracked.
4. All FFI bindings must pass `cargo geiger` audit.

### Alternatives to Unsafe

Before writing `unsafe`, consider:
- Safe abstractions from the standard library (e.g., `std::cell::UnsafeCell` is not needed when `std::sync::Mutex` suffices).
- Zero-copy parsing crates (`nom`, `winnow`) that avoid unsafe.
- The `bytemuck` crate for safe reinterpretation of bytes.
- The `pin-project` crate for safe pin projections.

---

## Documentation Standards

### rustdoc

All public items (functions, types, traits, methods, fields, constants) must have rustdoc comments.

```rust
/// Short summary line.
///
/// Detailed description. Explain what the function does, not how.
/// Include any preconditions, postconditions, error conditions, and
/// panics (though panics should be avoided).
///
/// # Arguments
/// * `input` - Description of the argument.
///
/// # Errors
/// Returns `CoreError::ConfigNotFound` if the key is not present.
///
/// # Examples
/// ```rust
/// # use ai_os_core::config::get_config;
/// let cfg = get_config("service.port").unwrap();
/// ```
fn get_config(key: &str) -> CoreResult<String> { ... }
```

### Required Sections

- `# Errors` — Required on all functions returning `Result`.
- `# Panics` — Document any remaining panic paths.
- `# Safety` — Required on all `unsafe` functions.
- `# Examples` — Encouraged for public API. Examples must compile (use `ignore` only when impossible).

### Module-Level Docs

Every module must have a module-level doc comment explaining its responsibility, key types, and relationship to other modules:

```rust
//! Task scheduling and dispatch.
//!
//! The scheduler accepts tasks, maintains priority queues, and
//! dispatches tasks to worker pools based on resource availability.
//! It communicates task lifecycle events on the EventBus.
//!
//! ## Key Types
//! - [`Scheduler`] — Central scheduler instance.
//! - [`TaskPriority`] — Priority levels for task ordering.
//!
//! ## Events Emitted
//! - `runtime.scheduler.task_dispatched`
//! - `runtime.scheduler.task_completed`
```

### Inline Comments

Use inline comments sparingly, and only to explain *why* a decision was made, not *what* the code does. The code should be self-explanatory for what it does.

---

## Module Structure

### File Organization

```
src/
  lib.rs          -- Crate root: re-exports, module declarations
  main.rs         -- Binary entry point (binary crates only)
  module_name/
    mod.rs        -- Module root: pub types, internal re-exports
    sub_module.rs -- Sub-modules
    tests.rs      -- Unit tests for this module
```

Each module is a single responsibility unit. Modules are placed in subdirectories when they contain multiple files.

### Crate Root (`lib.rs`)

The crate root should:
1. Declare all public modules with `pub mod`.
2. Re-export the most common types at the crate root.
3. Define the crate-level error type alias.
4. Provide the crate documentation comment (`//!`).

See `core/src/lib.rs` and `runtime/src/lib.rs` for reference implementations.

---

## Imports Organization

Imports are organized in groups, separated by a blank line, in this order:

1. Standard library (`use std::...`)
2. External crates (`use tokio::...`, `use serde::...`)
3. Workspace crates (`use ai_os_core::...`)
4. Local module (`use super::...`, `use crate::...`)

Within each group, imports are sorted alphabetically:

```rust
use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tracing::{info, span};

use ai_os_core::error::CoreError;
use ai_os_core::events::Event;

use crate::scheduler::Scheduler;
use crate::task::Task;
```

Wildcard imports (`use module::*`) are forbidden in production code. They are acceptable in test modules and prelude modules.

---

## Trait Design Guidelines

### Trait Definition

1. Traits define behavior, not data. Avoid default method implementations unless they represent true defaults (not convenience methods).
2. Keep traits focused. A trait with more than 5 methods should probably be split.
3. Prefer generic traits with associated types over concrete types in trait methods.
4. Use `#[async_trait]` for async trait methods (see Async Patterns below).

```rust
/// Provider of health check functionality.
#[async_trait]
pub trait HealthCheck: Send + Sync {
    /// Unique name for this health check.
    fn name(&self) -> &'static str;

    /// Perform the health check and return the result.
    async fn check(&self) -> HealthResult;
}
```

### Trait Bounds

1. Require only the bounds you need. Prefer `impl Trait` in function arguments over generic parameters when the trait is used only once.
2. Add `Send + Sync + 'static` bounds on trait objects that will be used across Tokio task boundaries.
3. Use `#[async_trait]` rather than return-position `impl Future` in traits for clarity (see async patterns).

### Trait Objects vs Generics

- Use generics (`fn process<T: Handler>(handler: T)`) when the concrete type is known at monomorphization time and there are many call sites.
- Use trait objects (`fn process(handler: Box<dyn Handler>)`) when heterogeneous storage or dynamic dispatch is needed.
- Document the performance trade-off when choosing trait objects in hot paths.

---

## Async Patterns

### Runtime

The project uses Tokio as its async runtime. All async code runs on Tokio's multi-threaded scheduler unless explicitly configured otherwise.

### Async Trait Methods

Use the `#[async_trait]` macro from the `async-trait` crate for trait methods that need to be async:

```rust
use async_trait::async_trait;

#[async_trait]
pub trait EventHandler: Send + Sync {
    async fn handle(&self, event: Event) -> Result<(), HandlerError>;
}
```

### Spawning Tasks

```rust
// Fire-and-forget
tokio::spawn(async move {
    process_event(event).await;
});

// With error handling
let handle = tokio::spawn(async move {
    if let Err(e) = process_event(event).await {
        error!(error = %e, "event processing failed");
    }
});
```

### Blocking Code

Wrap synchronous blocking operations with `tokio::task::spawn_blocking`:

```rust
let result = tokio::task::spawn_blocking(move || {
    // CPU-intensive or blocking I/O
    expensive_computation()
}).await??;
```

### Async Conventions

1. All I/O-bound functions are `async fn`.
2. Pure computation functions are synchronous.
3. Use `tokio::sync` primitives (`mpsc`, `oneshot`, `watch`, `Mutex`) rather than `std::sync` blocking primitives inside `.await` calls.
4. Hold `Mutex` guards across `.await` points only when necessary. Prefer message passing over shared state.
5. Use `Stream` and `Sink` traits for event streams.

### Cancellation

1. All async functions should handle cancellation gracefully (Tokio tasks are cancelable at `.await` points).
2. Use `tokio::select!` for timeouts and race conditions.
3. Use `CancellationToken` from `tokio_util` for cooperative cancellation.

---

## Testing Requirements

See the [testing guide](testing.md) for full details.

### Minimum Standards

- Every public function has at least one unit test covering the happy path and one covering the primary error path.
- Every module has a `#[cfg(test)] mod tests` block.
- Integration tests live in `tests/` at the crate root.
- All tests must pass before merging.

### Test Conventions

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_dispatch_event() {
        // Arrange
        let mut bus = EventBus::new();
        let mut rx = bus.subscribe("test.event");

        // Act
        bus.dispatch(Event::new("test.event", payload)).await;

        // Assert
        let event = rx.recv().await.unwrap();
        assert_eq!(event.event_type(), "test.event");
    }

    #[tokio::test]
    async fn test_no_handler_error() {
        let mut bus = EventBus::new();
        let result = bus.dispatch(Event::new("unknown", payload)).await;
        assert!(result.is_err());
    }
}
```

- Use `#[tokio::test]` for async tests.
- Prefer descriptive test names (`test_function_condition_expected`).
- Use Arrange-Act-Assert with blank line separators.

---

## Crate Dependency Guidelines

1. Minimize external dependencies. Before adding a crate, ask: "Can we implement this ourselves with reasonable effort?"
2. Prefer well-maintained, widely-used crates over niche alternatives.
3. Pin all dependencies to semver-compatible versions.
4. Keep the dependency tree lean. Run `cargo tree` to audit transitive dependencies.
5. Workspace crates depend on internal crates via `path = "../crate-name"`.

---

## Enforcement

The following CI checks enforce these standards:

| Check | Command | Failure Condition |
|---|---|---|
| Formatting | `cargo fmt --all -- --check` | Unformatted files |
| Linting | `cargo clippy --workspace --all-targets -- -D warnings` | Any clippy warning |
| Documentation | `cargo doc --workspace --no-deps` | Broken intra-doc links |
| Tests | `cargo test --workspace` | Failing tests |
| Audit | `cargo audit` | Vulnerability in dependencies |

These standards are maintained as living documents. Propose changes via pull request to this file with rationale.
