# AI Agent Instruction Manual

This file is the permanent instruction manual for all AI coding agents working on the AI-native OS project. Read this file before making any changes. Follow every rule. When in doubt, reference this file.

---

## Table of Contents

1. [Project Vision](#1-project-vision)
2. [Long-Term Goals](#2-long-term-goals)
3. [Coding Principles](#3-coding-principles)
4. [Architectural Principles](#4-architectural-principles)
5. [Module Responsibilities](#5-module-responsibilities)
6. [Folder Responsibilities](#6-folder-responsibilities)
7. [Rust Coding Standards](#7-rust-coding-standards)
8. [Testing Requirements](#8-testing-requirements)
9. [Documentation Requirements](#9-documentation-requirements)
10. [Git Workflow](#10-git-workflow)
11. [Branch Strategy](#11-branch-strategy)
12. [Commit Message Conventions](#12-commit-message-conventions)
13. [Review Checklist](#13-review-checklist)
14. [Performance Guidelines](#14-performance-guidelines)
15. [Security Guidelines](#15-security-guidelines)
16. [Rules for Introducing Dependencies](#16-rules-for-introducing-dependencies)
17. [Rules for Modifying Existing Modules](#17-rules-for-modifying-existing-modules)
18. [Rules for Creating New Modules](#18-rules-for-creating-new-modules)
19. [Definition of Done](#19-definition-of-done)

---

## 1. Project Vision

AI-native OS is an open-source operating platform built on Arch Linux that reimagines the relationship between the operating system and artificial intelligence. AI is not an application layer bolted onto a conventional OS; it is a first-class citizen throughout the system stack.

In traditional operating systems, the kernel manages hardware resources and provides abstractions (processes, files, sockets, signals) to user-space programs. AI, if at all present, runs as a user-space workload consuming those abstractions. In AI-native OS, the platform provides first-class primitives for intelligence: memory stores that persist learned patterns, perception pipelines that process sensor data, a brain layer that models state and makes decisions, and an execution layer that carries out those decisions across the system.

This is not an operating system that runs AI workloads. It is an operating system that thinks.

### Core Tenets

- **Intelligence is a platform primitive.** Memory, perception, reasoning, and execution are first-class abstractions provided by the operating platform, not application-layer concerns.
- **Clean layers with strict dependency direction.** Outer layers depend on inner layers. Inner layers never depend on outer layers.
- **Events are the universal communication medium.** Every component communicates through typed, traceable events on the EventBus. No direct references between components.
- **Security is designed in from the start.** Every interaction is authenticated, authorized, and audited. Default deny.
- **The system observes itself.** Logs, metrics, traces, and events are built into every component.

### Phase Status

| Phase | Name | Status |
|---|---|---|
| 1 | Development Environment | Completed |
| 2 | Core Platform | Completed |
| 3 | Runtime Platform | Completed |
| 4 | System Platform | Completed |
| 5 | Memory Platform | Completed |
| 6 | Brain Platform | Completed |
| 7 | Perception Platform | Completed |
| 8 | Execution Platform | Completed |
| 9 | Intelligence Integration | Completed |
| 10 | LIFE Platform (World Store / Open Space) | In Progress |

---

## 2. Long-Term Goals

1. **Ten-year horizon.** Build an operating platform that runs continuously for a decade without foundational rewrites.
2. **Self-optimizing system.** The platform observes its own performance and adapts scheduling, resource allocation, and cache policies without human intervention.
3. **Multi-node distribution.** Scale from single Arch Linux node to a cluster with distributed event bus, consensus, and workload migration.
4. **Plugin ecosystem.** Stable SDK for third-party extensions across any layer without modifying core code.
5. **Language-agnostic agent execution.** Support agents in Python, JavaScript, Rust, and others via WASM sandbox or subprocess with structured I/O.
6. **Secure by default.** Default configuration passes independent audits without changes.
7. **Observe everything.** Every event, decision, and state transition is recorded, traceable, and replayable.
8. **Human-in-the-loop when needed.** Critical decisions escalate to humans with full context; routine decisions are fully automated.

---

## 3. Coding Principles

1. **Clean Architecture.** Source code dependencies point inward. Outer layers depend on inner layers. Inner layers never depend on outer layers.
2. **Event-Driven Design.** All inter-module communication goes through the EventBus. No direct references to other modules' implementations.
3. **Fail Fast, Fail Gracefully.** Validate inputs at boundaries. Return errors on invariant violation. Never silently swallow errors.
4. **Security by Default.** Default configuration is the most secure. Permissions denied by default. Every public API entry point performs authorization.
5. **Observability First.** Every component produces structured logs, exposes metrics, and emits lifecycle events.
6. **Single Responsibility.** Each module, trait, struct, and function has exactly one well-defined responsibility. If you cannot describe it in one sentence, split it.
7. **Explicit Over Implicit.** Configuration, dependencies, and error paths are explicit. Avoid macros that obscure control flow.
8. **Async by Default.** All I/O, event handling, and inter-module calls are async (Tokio). CPU-bound work uses `tokio::task::spawn_blocking`.
9. **Immutable State Where Possible.** Prefer immutable data. Use `RwLock`/`Mutex` only when shared mutable state is unavoidable. Keep lock scopes minimal.
10. **Convention Over Configuration.** Name things consistently. New modules look like existing modules.
11. **Test-First for Logic.** Business logic, state machines, and security checks have unit tests before implementation.
12. **Documentation as Source of Truth.** Public APIs documented with rustdoc. Architecture decisions in ADRs. AGENTS.md is the definitive behavioral guide for AI agents.

---

## 4. Architectural Principles

### Layered Architecture

The platform has nine layers, each depending only on the layer directly beneath it:

```
Layer 8: Intelligence Integration          (future, Phase 9)
Layer 7: Execution Platform                (future, Phase 8)
Layer 6: Brain Platform                    (future, Phase 6)
Layer 5: Perception Platform               (future, Phase 7)
Layer 4: Memory Platform                   (future, Phase 5)
Layer 3: Runtime Platform                  (completed, Phase 3)
Layer 2: Core Platform                     (completed, Phase 2)
Layer 1: Operating System Abstraction      (in progress, Phase 4)
         Layer (OSAL)
         ═══════════════════════════════════════════
Layer 0: Linux Kernel / Hardware           (external)
```

The horizontal line below OSAL is the **platform boundary**. Everything above OSAL is OS-independent. All Linux-specific logic (syscalls, procfs, dbus, inotify, systemd, rtnetlink, libc) lives behind OSAL trait interfaces. No code in any layer above OSAL may reference Linux-specific types or call libc directly.

The boot process reads `platform.toml` (at the workspace root) to determine platform identity and which subsystems to enable. This file is loaded before any other initialization.

### EventBus

- Universal communication backbone. All inter-module communication passes through it.
- Events are Rust structs implementing `Event` trait: `fn event_type(&self) -> &'static str`.
- Subscribers register callbacks for specific event types (dispatched by `TypeId`).
- Event dispatch is async and non-blocking. Publishers never wait for subscribers.
- Events carry trace context for distributed tracing across module boundaries.

### Service Lifecycle

Every long-lived component implements the `Service` trait:

```rust
#[async_trait]
pub trait Service: Debug + Send + Sync {
    fn name(&self) -> &'static str;
    async fn init(&self) -> Result<(), CoreError>;
    async fn start(&self) -> Result<(), CoreError>;
    async fn stop(&self) -> Result<(), CoreError>;
}
```

States: `Initialized` -> `Running` -> `Stopped` -> `Failed`. Managed by `LifecycleManager` in Core.

### Module Independence

Each module is a separate Cargo crate in the workspace. Modules communicate exclusively through the EventBus. A module may depend on crates from lower layers but never on crates from higher layers. Circular dependencies between crates are forbidden.

---

## 5. Module Responsibilities

### Core (`core/` crate `ai_os_core`)

The foundational layer. No dependencies on other project crates.

| Submodule | Responsibility |
|---|---|
| `events` | Event trait, EventBus publish/subscribe, async dispatch, type-erased handler routing |
| `lifecycle` | Service trait, state machine (init/start/stop), LifecycleManager for coordinated lifecycle |
| `logging` | Structured JSON logging via tracing-subscriber, module-level filtering, multiple sinks |
| `health` | Health check registration, probe execution, 3-state status (Healthy/Degraded/Unhealthy) |
| `container` | String-keyed dependency injection registry with singleton scope |
| `config` | Configuration loading from JSON/env/layered sources |
| `registry` | Name-based service directory for runtime lookup |
| `bootstrap` | PlatformBuilder that wires all Core subsystems together |
| `application` | Top-level Platform handle with run_until_signal |

### Runtime (`runtime/` crate `ai_os_runtime`)

Depends on Core. Task lifecycle, scheduling, and agent management.

| Submodule | Responsibility |
|---|---|
| `scheduler` | Priority queue (BinaryHeap), FIFO tiebreaker, enqueue/dequeue/peek/remove, SchedulerStats |
| `supervisor` | Restart policies (Never/OnFailure/Always), retry tracking, TaskRestarting event |
| `session` | Session CRUD, permission binding, session state |
| `task` | Task CRUD, state machine (Pending/Running/Completed/Failed/Cancelled), Priority (0-100) |
| `context` | Trace/span propagation via tokio::task_local!, Context struct (trace_id, span_id, metadata) |
| `state` | RuntimePhase state machine (6 states, 8 transitions), transition validation |
| `resource` | Per-task resource usage tracking, saturating CPU clamp, limit enforcement |
| `permission` | Role-based access control (admin/user/readonly), permission sets |

### OSAL — Operating System Abstraction Layer (`system/` — Phase 4, in progress)

The lowest internal layer of the platform. Depends on nothing except the Linux kernel and standard Rust crates. Core and Runtime depend on OSAL, not the reverse. All Linux-specific logic is confined to this layer.

| Planned Submodule | Responsibility |
|---|---|
| `system_resource` | CPU/memory/disk monitoring via /proc and sysinfo |
| `process` | Process spawn, signal, wait via libc |
| `filesystem` | File read/write/stat/watch via std::fs and inotify |
| `network` | Network interfaces, sockets, DNS via rtnetlink/trust-dns |
| `event_collector` | udev/dbus signal to EventBus bridge |
| `service_manager` | systemd service CRUD via dbus |

### Future Modules

`memory/` (Phase 5), `brain/` (Phase 6), `perception/` (Phase 7), `execution/` (Phase 8) exist as placeholders only. Do not implement functionality in them yet.

---

## 6. Folder Responsibilities

| Path | Purpose |
|---|---|
| `/` | Workspace root. `Cargo.toml` defines workspace members. |
| `core/` | Core Platform crate (`ai_os_core`). Foundational services. |
| `runtime/` | Runtime Platform crate (`ai_os_runtime`). Task lifecycle and agent management. |
| `system/` | OSAL crate placeholder (Phase 4). Operating System Abstraction Layer. |
| `memory/` | Memory Platform crate placeholder (Phase 5). |
| `brain/` | Brain Platform crate placeholder (Phase 6). |
| `perception/` | Perception Platform crate placeholder (Phase 7). |
| `execution/` | Execution Platform crate placeholder (Phase 8). |
| `docs/` | All documentation: ADRs, architecture, diagrams, modules, process docs. |
| `configs/` | Platform configuration files (JSON, env examples). |
| `scripts/` | Build scripts, CI scripts, dev tooling. |
| `tests/` | Cross-crate integration tests. |
| `tools/` | Standalone tooling (benchmark runners, diagnostics). |
| `assets/` | Static assets (keys, defaults, embedded resources). |
| `services/` | Systemd unit files and deployment service definitions. |
| `.github/` | GitHub Actions workflows, issue templates, PR templates. |

---

## 7. Rust Coding Standards

### Naming

| Category | Convention | Example |
|---|---|---|
| Types, traits, enums | `UpperCamelCase` | `TaskManager`, `Scheduler` |
| Functions, methods, variables | `snake_case` | `enqueue_task`, `current_retries` |
| Constants | `SCREAMING_SNAKE_CASE` | `MAX_RETRIES` |
| Module names | `snake_case` (single word preferred) | `scheduler`, not `task_scheduler` |
| Crate names (Cargo.toml) | `kebab-case` | `ai-os-core` |
| Crate names (Rust) | `snake_case` | `ai_os_core` |
| Type parameters | Short uppercase (`T`, `E`) | Descriptive for complex bounds (`Event`) |

### Formatting

- Enforced by `rustfmt` with default settings. Run `cargo fmt` before every commit.
- Maximum line length: 100 characters. Use 4-space indentation, no tabs.
- No trailing whitespace.

### Linting

- `cargo clippy` must pass with zero warnings.
- Allow attributes on individual items are strongly discouraged. If used, add a comment explaining why.
- CI uses `RUSTFLAGS="-D warnings"` to deny warnings.

### Error Handling

- Every crate defines its own error type implementing `std::error::Error`.
- Use `thiserror` for error type derivation. Use `anyhow` in binaries and integration tests only.
- Prefer specific error variants over generic catch-alls. Errors carry context about what failed and why.
- Never `unwrap()` or `expect()` in production code. Use `?` with error conversion or handle explicitly.
- Safe `.unwrap()` is permitted in test code only.

```rust
#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    #[error("task {0} not found")]
    TaskNotFound(TaskId),
    #[error("lock poisoned: {0}")]
    LockPoisoned(String),
}
```

### Unsafe Code

- `unsafe` is banned in application code. `#![forbid(unsafe_code)]` in every crate `lib.rs` by default.
- The only exception is FFI with the Linux kernel (Phase 4 System Platform). Must be isolated in a single `sys` module per crate, reviewed line-by-line, and documented with `// SAFETY:` comments.

### Module Structure

- Each module is a directory with `mod.rs` (public API, re-exports, tests).
- Internal implementation goes in sibling files: `scheduler/mod.rs` (pub trait), `scheduler/priority.rs` (impl).
- Keep modules small. If a module exceeds 500 lines, split it into submodules.
- `mod.rs` primarily re-exports. Put logic in named submodules.

```rust
// scheduler/mod.rs
mod priority_queue;
pub use priority_queue::PriorityScheduler;

#[cfg(test)]
mod tests;
```

### Imports

Group imports in this order, separated by blank lines:
1. `std` and `core`
2. External crates (tokio, serde, chrono, etc.)
3. `crate` and `super` local imports

Use `use` for paths, not inline paths. Prefer `use crate::module::Type` over `use super::super::module::Type`.

### Traits

- All traits that cross module boundaries must be `Send + Sync + Debug`.
- All traits intended for dynamic dispatch must be dyn-compatible: no generic methods, no `Self: Sized` bounds, no associated consts.
- Provide a `Default*` implementation for every trait in the same module.
- Default implementations return `Err(...)` or default values, never panic.

```rust
pub trait Scheduler: Debug + Send + Sync {
    fn enqueue(&self, task: TaskHandle) -> Result<(), RuntimeError>;
    fn dequeue(&self) -> Option<TaskHandle>;
}
```

### Async

- Use `#[async_trait]` from `async-trait` crate for async trait methods.
- All async functions use `tokio` as the runtime. Use `tokio::test` for async tests.
- CPU-bound work must use `tokio::task::spawn_blocking`. Never block the async runtime.
- Lock acquisitions (`lock()`, `read()`, `write()`) must never be held across `.await` points unless using `tokio::sync` primitives.

### Derives

Every struct and enum that participates in the public API should derive (where applicable): `Debug`, `Clone`, `Serialize`, `Deserialize`.

If a type must be `Copy`, verify that its fields are all `Copy` and the semantics are value-like.

### Common Patterns

- Use `Arc<dyn Trait>` for shared ownership of trait objects.
- Use `RwLock<T>` for read-heavy shared state. Use `Mutex<T>` for write-heavy state.
- Use `HashMap` backed by `RwLock` for lookup tables. Prefer `tokio::sync::RwLock` when held across `.await`.
- Implement `Default` for structs where a sensible default exists.
- Use `Result<T, Error>` for fallible operations. Use `Option<T>` for absent values.

---

## 8. Testing Requirements

### Test Types

| Type | Location | Purpose |
|---|---|---|
| Unit tests | `#[cfg(test)] mod tests` at bottom of each implementation file | Test individual functions and state transitions |
| Integration tests | `tests/` directory in each crate (e.g., `runtime/tests/`) | Test end-to-end flows through multiple subsystems |
| Cross-crate tests | `/tests/` at workspace root | Test interactions between crates |
| Benchmarks | `benches/` directory in each crate | Criterion benchmarks for performance-critical paths |
| Doc tests | `///` code blocks in rustdoc | Verify API examples compile and produce expected output |

### Rules

- Every public function must have at least one unit test covering the happy path.
- Every error variant must have at least one test covering the error path.
- State machines must have tests for every valid and invalid transition.
- Integration tests must use the same module setup as production (real traits, real impls, not mocks).
- Mocking external services (filesystem, network, dbus) is acceptable. Mocking internal modules is not.
- Tests must be deterministic. No sleeps, no timeouts, no race conditions. Use `tokio::time::pause()` when time-dependent behavior must be tested.
- Each test function tests exactly one behavior. Name tests clearly: `test_name` describes the scenario.
- Property-based testing (proptest) is encouraged for serialization, state machines, and arithmetic.

### Running Tests

```bash
# All tests across all crates
cargo test --workspace

# Single crate
cargo test -p ai-os-runtime

# Single test
cargo test -p ai-os-runtime -- scheduler::tests::high_priority_comes_first

# With output
cargo test -- --nocapture
```

### Coverage

- Target: 80%+ line coverage for active crates (core, runtime).
- Use `cargo tarpaulin` or `grcov` for coverage reporting.
- Coverage gates in CI: PRs must not decrease overall coverage.
- Benchmark tests do not count toward coverage requirements.

### Benchmarks

- Use Criterion for all benchmarks.
- Every public trait method that is performance-sensitive should have a benchmark.
- Run benchmarks on dedicated hardware (or CI runner) to avoid variance.
- Benchmark results are stored in `benches/` as baselines.

---

## 9. Documentation Requirements

### What Must Be Documented

- Every `pub` item in every crate: modules, traits, structs, enums, functions, methods, consts.
- Every public error variant: document when it occurs and how callers should handle it.
- Every trait: document purpose, contract, and which implementations exist.
- Every module: document purpose, usage, and cross-links to related modules.

### Documentation Standards

- Module-level docs go at the top of `mod.rs` or `lib.rs` as `//!` comments.
- Item-level docs go before the item as `///` comments.
- Use markdown in doc comments. Code blocks in docs should be valid Rust or marked with the language.
- Cross-link between docs using `[`TypeName`]`. Doc `/// Links to [`EventBus`].`
- Keep doc comments concise. Explain what the function does, not how it does it.
- Examples in doc comments must compile and run (they become doc tests).

### Request for Comments (RFCs)

- Every significant change or new module must have an accepted RFC in `docs/rfc/` before implementation begins.
- Use the template at `docs/rfc/template.md`.
- RFC statuses: Draft → Review → Accepted / Rejected / Postponed.
- RFCs propose what to build and why. ADRs record what was actually decided during implementation.
- Current RFCs: 0001 (System Platform), 0002 (Memory Platform), 0003 (Brain Platform).

### Architecture Decision Records

- Every significant architectural decision gets an ADR in `docs/adr/`.
- Use the template at `docs/adr/template.md`.
- ADRs are never deleted. When a decision is superseded, create a new ADR and update the old one's status.
- Current ADRs: 0001 (Vision), 0002 (Clean Architecture), 0003 (Rust), 0004 (Event-Driven), 0005 (Runtime).

### AGENTS.md

- This file is the definitive behavioral guide for AI coding agents.
- Read it before any session. Follow all rules. When in doubt, reference this file.
- This file may be updated by humans only. AI agents may suggest updates but must not modify it directly.

---

## 10. Git Workflow

### General Rules

- Never commit directly to `main`. All changes go through feature branches and pull requests.
- Keep commits atomic: one logical change per commit. A commit should be a coherent unit that could be reverted independently.
- Rebase feature branches onto `main` before creating a PR. Merge commits in `main` are acceptable.
- Keep branch names consistent with the branch strategy below.
- Sign commits with GPG or SSH when possible.

### Pull Requests

- Every PR must reference an issue.
- PR title follows conventional commit format (see section 12).
- PR description must contain: what changed, why it changed, how to verify, and links to related issues/ADRs.
- A PR must pass CI (build, clippy, tests, coverage) before merging.
- Self-review first: re-read your diff before requesting review.

---

## 11. Branch Strategy

| Branch Pattern | Purpose | Source |
|---|---|---|
| `main` | Stable, production-ready code. Protected. | N/A |
| `feature/<name>` | New features for any phase | `main` |
| `fix/<name>` | Bug fixes | `main` |
| `docs/<name>` | Documentation-only changes | `main` |
| `refactor/<name>` | Code restructuring without behavior change | `main` |
| `release/v<major>.<minor>` | Release preparation | `main` |

### Rules

- `main` is protected. No direct pushes. All merges via PR.
- Feature branches should be short-lived (days, not weeks). Rebase frequently.
- Delete the branch after merging.
- Phase-level feature branches (`feature/runtime`, `feature/memory`) are for coordinating multi-PR work toward a phase goal. They merge back to `main` when the phase is complete.

---

## 12. Commit Message Conventions

Use Conventional Commits format (https://www.conventionalcommits.org/):

```
<type>(<scope>): <description>

[optional body]

[optional footer]
```

### Types

| Type | Usage |
|---|---|
| `feat` | A new feature or module |
| `fix` | A bug fix |
| `docs` | Documentation only |
| `refactor` | Code restructuring (no behavior change) |
| `test` | Adding or fixing tests |
| `bench` | Adding or updating benchmarks |
| `perf` | Performance improvement |
| `style` | Formatting, linting, whitespace (no semantic change) |
| `chore` | Build system, CI, dependency updates |
| `revert` | Reverting a previous commit |

### Scope

Scope is the crate or module being changed: `core`, `runtime`, `system`, `docs`, `ci`, `workspace`.

### Examples

```
feat(scheduler): implement priority queue with BinaryHeap

Add priority-based task scheduling using BinaryHeap. Higher-priority
tasks (lower numeric value) are dequeued first. Equal-priority tasks
use FIFO ordering via created_at timestamp tiebreaker.

Closes #42
```

```
fix(runtime): correct RestartPolicy retry counting

Change should_restart from strict inequality (<) to
inclusive (<=) so that max_retries counts the actual
number of retries performed.

Fixes #58
```

```
docs(architecture): add Runtime Platform deep-dive
```

### Rules

- First line: max 72 characters. Capitalized. No period.
- Body: wrapped at 72 characters. Explain what and why, not how.
- Footer: reference issues (`Closes #42`, `Fixes #58`, `Refs #10`).
- Use imperative mood: "add", "fix", "implement" — not "added", "fixes", "implemented".

---

## 13. Review Checklist

Before submitting a PR, verify every item:

### Correctness

- [ ] Does the code do what the specification says?
- [ ] Are all error paths handled? Are error types appropriate?
- [ ] Are edge cases (empty input, maximum values, concurrent access) handled?
- [ ] Do all existing tests still pass?

### Design

- [ ] Does the change follow clean architecture? Are dependency directions correct?
- [ ] Are traits dyn-compatible? Are they `Send + Sync`?
- [ ] Is the public API minimal? Are implementation details private?
- [ ] Are lock scopes as narrow as possible?
- [ ] Could this be split into smaller commits?

### Style

- [ ] Does `cargo fmt` produce no changes?
- [ ] Does `cargo clippy` produce zero warnings?
- [ ] Are all imports grouped and sorted correctly?
- [ ] Are there no `unwrap()` or `expect()` calls in production code?
- [ ] Are there no `unsafe` blocks unless explicitly justified?

### Testing

- [ ] Are new features covered by unit tests?
- [ ] Are there integration tests for end-to-end flows?
- [ ] Do error paths have test coverage?
- [ ] Are benchmarks added/updated for performance-sensitive code?

### Documentation

- [ ] Do all new public items have rustdoc comments?
- [ ] Are module-level docs updated?
- [ ] Is the ADR updated or a new ADR created if the architecture changed?
- [ ] Are cross-links in docs correct and not broken?

### Safety & Security

- [ ] Are all public entry points authorized?
- [ ] Are inputs validated at boundaries?
- [ ] Are there no secrets or credentials in the code?
- [ ] Are file paths sanitized? Are command injections prevented?

---

## 14. Performance Guidelines

### General

- Profile before optimizing. Use `perf`, `flamegraph`, or `tokio-console` to identify bottlenecks.
- Prefer efficient algorithms (O(n log n) over O(n^2)). Measure, don't guess.
- Minimize allocations in hot paths. Use `Vec::with_capacity`, reuse buffers via `BytesMut`.
- Avoid `clone()` in hot paths. Prefer `Arc` or references.
- Use `#[inline]` only when profiling shows it matters.

### Async Performance

- Never block the async runtime. Use `spawn_blocking` for CPU-bound or blocking I/O.
- Keep critical sections short. Never hold a lock across `.await`.
- Use bounded channels (`tokio::sync::mpsc`) for work queues to prevent unbounded memory growth.
- Prefer `tokio::sync::RwLock` over `std::sync::RwLock` when the lock is held across `.await`.

### Memory

- Prefer stack allocation over heap allocation.
- Use `Box<[T]>` or `Vec<T>` instead of `Box<dyn Trait>` when the concrete type is known at call sites.
- Monitor memory usage with `procinfo` or `/proc/self/status`. Set `RLIMIT_AS` for resource containment.
- Use `#[derive(Clone)]` judiciously. Large types should implement Clone only when necessary.

### Data Structures

- `HashMap` for unordered lookups. `BTreeMap` for ordered iteration.
- `BinaryHeap` for priority queues (used by Scheduler).
- `Vec` as the default sequence type. Pre-allocate with `Vec::with_capacity` when the size is known.

### Concurrency

- Use `Arc<dyn Trait>` for shared trait objects.
- Use `RwLock` for read-heavy, write-rare state. Use `Mutex` for write-heavy state.
- Prefer `AtomicUsize`/`AtomicBool` for simple counters and flags over locks.
- Use `tokio::sync::Notify` or `watch` channels for one-to-many notifications.

---

## 15. Security Guidelines

### Authentication and Authorization

- Every public API entry point must perform authorization.
- Default policy: deny all. Explicitly grant permissions.
- Use the `PermissionChecker` trait (Runtime) for all authorization decisions.
- Permissions are role-based: `Admin` (all), `User` (read/write own), `ReadOnly` (read only).
- Sessions bind permissions to identities. Always validate session permissions before acting.

### Input Validation

- Validate all inputs at system boundaries. Reject malformed input early.
- For filesystem paths: canonicalize and verify the resolved path is within allowed directories.
- For command execution: use structured arguments (arrays), never shell string interpolation.
- For network input: validate length, encoding, and schema before processing.

### Secrets Management

- No secrets in source code. No hardcoded passwords, API keys, or tokens.
- Secrets are loaded from environment variables, encrypted config files, or a secrets vault.
- Secrets are never logged. Use `#[serde(skip_serializing)]` on secret fields.
- Never commit `.env` files or credential files to the repository.

### Filesystem Security

- Follow the principle of least privilege: open files with the minimum required permissions.
- Use `O_CLOEXEC` for file descriptors passed to child processes.
- Canonicalize paths before access to prevent symlink traversal attacks.
- Validate that file paths are within permitted data directories.

### Dependency Security

- Run `cargo audit` regularly (CI gate). No dependencies with known vulnerabilities.
- Pin dependency versions in `Cargo.lock`. Review dependency diffs on updates.
- Minimize the dependency tree. Each dependency is an attack surface increase.
- Reject dependencies with unclear licensing or provenance.

### Logging and Monitoring

- Log all authentication and authorization decisions.
- Log all security-relevant events: permission denials, failed logins, unexpected errors.
- Never log Personally Identifiable Information (PII) or secrets.
- Security events have a distinct log level (`WARN` for failures, `ERROR` for breaches).

---

## 16. Rules for Introducing Dependencies

### Pre-Approval Required

Every new dependency must go through a lightweight approval process. The following information must be provided and documented in the PR:

1. **Purpose.** What problem does this dependency solve that cannot be solved with existing code or stdlib?
2. **Alternatives considered.** What other crates were evaluated? Why was this one chosen?
3. **License.** Must be MIT, Apache 2.0, BSD-2/3, or CC0. No GPL/LGPL/AGPL in library crates.
4. **Safety.** Is the crate `#![forbid(unsafe_code)]`? If it uses unsafe, what for and is it sound?
5. **Audit status.** Run `cargo audit` on the crate and all transitive dependencies. No known vulnerabilities.
6. **Maintenance.** Is the crate actively maintained? Last release date, number of downloads, number of dependents.

### Hard Rules

- No dependencies on GPL/LGPL/AGPL licensed crates in library code.
- No dependencies on unmaintained crates (no release in 2+ years).
- No dependencies with known security vulnerabilities.
- No dependency on `unsafe` code unless the unsafe is:
  - Confined to a small `sys` module
  - Reviewed and documented with `// SAFETY:` comments
  - Unavoidable (FFI, SIMD, low-level memory)
- No `dev-dependencies` that are not strictly needed for testing or benchmarking.

### Dependency Tree Hygiene

- Prefer small, focused crates over large frameworks.
- Avoid duplicate dependencies (same crate at different versions) in the workspace.
- Use workspace-level dependencies in `[workspace.dependencies]` to ensure consistent versions across crates.
- Audit dependencies on every major update. Pin minor versions for stability.

### Current Approved Dependencies

| Crate | Purpose | Approved |
|---|---|---|
| `tokio` | Async runtime | Phase 1 |
| `serde` + `serde_json` | Serialization | Phase 1 |
| `tracing` + `tracing-subscriber` | Structured logging | Phase 1 |
| `chrono` | Timestamps | Phase 2 |
| `async-trait` | Async trait methods | Phase 2 |
| `thiserror` | Error type derivation | Phase 2 |
| `anyhow` | Error handling (binaries/test only) | Phase 2 |
| `uuid` | ID generation | Phase 2 |
| `criterion` | Benchmarks (dev-dependency) | Phase 3 |
| `tokio-test` | Async test utilities (dev-dependency) | Phase 3 |

---

## 17. Rules for Modifying Existing Modules

### Before You Modify

1. Read the module's documentation (`docs/modules/` and `docs/architecture/`).
2. Read the trait definition and understand the contract.
3. Check if there are related ADRs that constrain the design.
4. Check the existing tests to understand expected behavior.
5. Read the file in question completely before editing.

### Modification Rules

- **Never change a trait signature** without updating all implementations and tests, and creating a new ADR if the change affects cross-module contracts.
- **Never remove a public API** without a deprecation period. Mark deprecated items with `#[deprecated]` and a note about the replacement.
- **Never change behavior silently.** If fixing a bug, add a test that fails before the fix and passes after.
- **Preserve the module's contract.** A module that implements `Scheduler` must remain a scheduler. Do not add unrelated responsibilities.
- **Keep trait methods dyn-compatible.** No adding generic parameters to trait methods.
- **Match existing code style.** New code in `scheduler/mod.rs` must look like the existing code in `scheduler/mod.rs`.
- **Update tests.** When modifying logic, add tests for the new behavior and ensure existing tests still pass.
- **Update docs.** Module-level docs, rustdoc on changed items, and cross-references must be kept in sync.

### When to Refactor

- When a module exceeds 500 lines, split it into submodules.
- When a function exceeds 50 lines, extract helper functions.
- When a struct has more than 10 fields, consider grouping related fields into sub-structs.
- When match statements grow beyond 8 arms, consider a strategy pattern.

### After Modification

- Run `cargo fmt` and `cargo clippy`. Zero warnings.
- Run all tests for the affected crate and all dependent crates.
- Run `cargo test --workspace` to ensure nothing else broke.
- If any test was added/modified, run it individually to confirm it tests the right thing.

---

## 18. Rules for Creating New Modules

### Pre-Requisites

1. A new module must correspond to a phase in the roadmap.
2. A new module must have an ADR (or reference an existing ADR) explaining the architectural decision.
3. A new module must have module-level documentation before implementation begins: purpose, responsibilities, interfaces, dependencies, events.
4. A new module must be a separate Cargo crate in the workspace.

### Naming and Structure

- Crate name: `ai-os-<name>` (e.g., `ai-os-system`).
- Directory: `<name>/` (e.g., `system/`).
- Rust identifier: `ai_os_<name>` (e.g., `ai_os_system`).
- Module structure follows the same pattern as existing crates:
  - `Cargo.toml` with `edition = "2021"`, `license = "MIT"`.
  - `src/lib.rs` with `#![forbid(unsafe_code)]` and `#![warn(missing_docs)]`.
  - `src/error.rs` with the crate's error type.
  - `src/<submodule>/mod.rs` for each public submodule.

### Dependency Rules

- A module at layer N may depend on any module at layer < N.
- A module at layer N must NOT depend on any module at layer > N.
- A module at layer N may depend on Core (layer 2) for EventBus, Service, Logger.
- A module at layer N may depend on Runtime (layer 3) for task, session, permission, scheduler.
- A module at layer N must NOT depend on System (layer 1) — wait, this is backward. Let me clarify:
  - System (layer 1) depends on nothing.
  - Core (layer 2) depends on System.
  - Runtime (layer 3) depends on Core.
  - Memory (layer 4) depends on Runtime + Core + System.
  - etc.

Actually, the layering is: System (bottom), then Core, Runtime, Memory, Perception, Brain, Execution, Intelligence (top).

System depends on nothing (except the OS).
Core depends on System.
Runtime depends on Core.
Memory depends on Runtime.
Perception depends on Memory.
Brain depends on Perception.
Execution depends on Brain.
Intelligence depends on Execution.

### Requirements for a New Crate

Every new crate must include:

```
<name>/
├── Cargo.toml
├── src/
│   ├── lib.rs          # crate docs, #![forbid(unsafe_code)], pub mod declarations
│   ├── error.rs        # Error enum with thiserror
│   └── <submodule>/
│       └── mod.rs      # trait, DefaultImpl, #[cfg(test)] mod tests
├── tests/
│   └── integration.rs  # end-to-end tests
└── benches/
    └── <name>_bench.rs # Criterion benchmarks
```

### Trait Design for New Modules

- Every public capability is defined as a trait.
- All traits are `Debug + Send + Sync`.
- All traits are dyn-compatible (no generic methods).
- Each trait has a `Default*` implementation.
- Each trait has at least one test.

### Events for New Modules

- Events are structs defined in the module that implement the `Event` trait from Core.
- Events are published on the Core EventBus.
- Events carry trace context (via ContextManager) for distributed tracing.
- Event names follow the convention: `<module>.<event_name>` (e.g., `system.process_started`).

### Integration with Existing Modules

- New modules register themselves as Service implementations with the Core LifecycleManager.
- New modules subscribe to relevant events during their `init()` phase.
- New modules publish their status changes as events.
- When the module starts, it emits a `<module>.started` event. When it stops, it emits `<module>.stopped`.

### Tests for New Modules

- Unit tests cover all trait methods: happy paths, error paths, edge cases.
- Integration tests cover at least one end-to-end flow through the module.
- Criterion benchmarks cover the performance-critical paths.
- Doc tests cover public API examples.

---

## 19. Definition of Done

A task is Done only when ALL of the following conditions are met:

### Code

- [ ] The implementation follows clean architecture — dependency direction is correct, no circular dependencies.
- [ ] Code compiles without errors or warnings (`cargo build --workspace`).
- [ ] `cargo fmt` produces no changes.
- [ ] `cargo clippy` produces zero warnings.
- [ ] All public items have rustdoc comments.
- [ ] No `unsafe` blocks (unless explicitly justified and documented).
- [ ] No `unwrap()` or `expect()` in production code.
- [ ] Error types are specific and informative.
- [ ] Traits are `Send + Sync + Debug` and dyn-compatible.

### Tests

- [ ] Unit tests pass for the affected crate (`cargo test -p ai-os-<name>`).
- [ ] Integration tests pass for the affected crate.
- [ ] All workspace tests pass (`cargo test --workspace`).
- [ ] New code has unit tests covering at least the happy path and main error paths.
- [ ] Benchmarks compile and run without errors (if applicable).

### Documentation

- [ ] New public API items have rustdoc comments.
- [ ] Module-level documentation is updated (if module docs exist).
- [ ] ADR is created or updated if the change affects architecture.
- [ ] Cross-references in docs are correct (relative paths resolve).

### Integration

- [ ] The change works with existing modules. No regressions.
- [ ] Events published by new code are documented (event type, payload, when emitted).
- [ ] Events consumed by new code are documented (event type, expected payload, behavior).

### Git

- [ ] Commits are atomic and follow conventional commit format.
- [ ] Branch name follows the branch strategy.
- [ ] PR description explains what changed, why, and how to verify.

### Security

- [ ] No secrets in the code.
- [ ] Public API entry points are authorized.
- [ ] Inputs are validated at boundaries.
- [ ] File system paths are canonicalized before access (if applicable).
