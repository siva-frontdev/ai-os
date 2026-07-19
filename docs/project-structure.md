# Project Structure

## Overview

The AI-OS project is a Cargo workspace containing two published crates (`core`, `runtime`) and several scaffolded directories for future phases. The workspace is configured in the root `Cargo.toml`:

```toml
[workspace]
resolver = "2"
members = ["core", "runtime"]

[workspace.package]
version = "0.1.0"
edition = "2021"
license = "MIT"
```

---

## Directory Tree

```
ai-os/
├── Cargo.toml              -- Workspace manifest
├── Cargo.lock              -- Dependency lockfile
├── LICENSE                 -- MIT license
├── README.md               -- Project overview
├── CONTRIBUTING.md         -- Contribution guide (root-level)
│
├── core/                   -- Core Platform crate (Phase 2)
│   ├── Cargo.toml          -- ai-os-core crate manifest
│   ├── README.md           -- Crate-specific readme
│   ├── src/
│   │   ├── lib.rs          -- Crate root: module declarations, re-exports
│   │   ├── error.rs        -- CoreError enum definition
│   │   ├── application/    -- Top-level platform handle
│   │   ├── bootstrap/      -- Builder: wires all subsystems together
│   │   ├── config/         -- Configuration sources (JSON, env, layered)
│   │   ├── container/      -- Type-based dependency injection container
│   │   ├── events/         -- In-process pub/sub EventBus
│   │   ├── health/         -- Health checks and monitoring
│   │   ├── lifecycle/      -- Service lifecycle state machine
│   │   ├── logging/        -- Structured logging (tracing-based)
│   │   ├── registry/       -- Name-based service directory
│   │   └── utils/          -- Shared helpers (ID generation, timestamps)
│   └── tests/              -- Integration tests for core
│
├── runtime/                -- Runtime Platform crate (Phase 3)
│   ├── Cargo.toml          -- ai-os-runtime crate manifest
│   ├── src/
│   │   ├── lib.rs          -- Crate root: module declarations, re-exports
│   │   ├── error.rs        -- RuntimeError enum definition
│   │   ├── runtime.rs      -- Top-level Runtime facade
│   │   ├── context/        -- Trace context propagation
│   │   ├── permission/     -- Authorization and permission checks
│   │   ├── resource/       -- Resource tracking and limits
│   │   ├── scheduler/      -- Task scheduling and worker pools
│   │   ├── session/        -- Session lifecycle management
│   │   ├── state/          -- Runtime phase state machine
│   │   ├── supervisor/     -- Fault tolerance and restart policies
│   │   └── task/           -- Task creation, lifecycle, tracking
│   ├── tests/              -- Integration tests for runtime
│   └── benches/
│       └── runtime_bench.rs -- Criterion benchmarks
│
├── system/                 -- System Platform (Phase 4, in progress)
│   └── .gitkeep
│
├── memory/                 -- Memory Platform (Phase 5, planned)
│   └── .gitkeep
│
├── brain/                  -- Brain Platform (Phase 6, planned)
│   └── .gitkeep
│
├── perception/             -- Perception Platform (Phase 7, planned)
│   └── .gitkeep
│
├── execution/              -- Execution Platform (Phase 8, planned)
│   └── .gitkeep
│
├── services/               -- Platform services (Phase 4+)
│   └── .gitkeep
│
├── tools/                  -- Development and CLI tools
│   └── .gitkeep
│
├── scripts/                -- Build and utility scripts
│   └── setup.sh            -- Development environment setup
│
├── tests/                  -- Workspace-level integration tests
│   └── .gitkeep
│
├── configs/                -- Configuration files
│   └── .gitkeep
│
├── assets/                 -- Static assets
│   └── .gitkeep
│
└── docs/                   -- Project documentation
    ├── coding-standards.md
    ├── contributing.md
    ├── project-structure.md
    ├── build.md
    ├── development.md
    ├── testing.md
    ├── release.md
    ├── security.md
    ├── deployment.md
    ├── architecture.md
    ├── principles.md
    ├── glossary.md
    ├── roadmap.md
    ├── vision.md
    ├── adr/                 -- Architecture Decision Records
    ├── api/                 -- API reference (generated)
    ├── decisions/           -- Design decision records
    ├── diagrams/            -- Architecture diagrams (Mermaid, PNG)
    └── modules/             -- Module-specific documentation
```

---

## Crate Responsibilities

### `core/` — Core Platform (`ai-os-core`)

**Dependencies**: Rust standard library, external crates only. No internal workspace dependencies.

The Core Platform provides the foundational abstractions that every other layer builds upon. It has no knowledge of the layers above it.

| Module | Responsibility |
|---|---|
| `events` | Typed event bus with publish/subscribe, middleware pipeline, wildcard pattern matching |
| `lifecycle` | Standardized service state machine (Init, Starting, Running, Stopping, Stopped) |
| `logging` | Structured logging via the `tracing` crate with configurable sinks and levels |
| `health` | Liveness and readiness probe registry, periodic health check execution |
| `container` | Container runtime abstraction (Docker/Podman API integration) |
| `registry` | Name-to-service mapping for service discovery |
| `config` | Layered configuration merge from files, environment, and runtime events |
| `bootstrap` | Builder pattern that constructs and wires the complete platform |
| `application` | Top-level handle representing the running platform |
| `utils` | Utility functions (UUID generation, timestamp helpers, etc.) |
| `error` | `CoreError` enum covering all core failure modes |

**Key exports**: `CoreError`, `CoreResult`, `PlatformBuilder`, `EventBus`, `ServiceLifecycle`, `HealthMonitor`, `Logger`.

### `runtime/` — Runtime Platform (`ai-os-runtime`)

**Dependencies**: `ai-os-core` (path dependency), external crates.

The Runtime Platform provides the operational infrastructure for scheduling, supervision, sessions, tasks, state management, resource control, and permission enforcement.

| Module | Responsibility |
|---|---|
| `scheduler` | Multi-threaded work-stealing task dispatch with priority queues |
| `supervisor` | Service health monitoring and configurable restart policies |
| `session` | User and application session lifecycle, identity binding |
| `task` | Task creation, state tracking, worker dispatch |
| `context` | Distributed trace/span context propagation across async boundaries |
| `state` | Runtime state machine for phase transitions |
| `resource` | CPU, memory, and I/O resource tracking and limit enforcement |
| `permission` | Role-based permission checking and access control |
| `runtime` | Top-level `Runtime` facade that initializes and orchestrates all modules |
| `error` | `RuntimeError` enum covering all runtime failure modes |

**Key exports**: `RuntimeError`, `RuntimeResult`, `Runtime`, `Scheduler`, `Supervisor`, `SessionManager`, `PermissionChecker`.

### Future Crates

| Directory | Phase | Planned Modules |
|---|---|---|
| `system/` | 4 (in progress) | DaemonManager, PolicyEngine, CapabilityDiscovery, ConfigManager, PluginSystem |
| `memory/` | 5 | PersistentStore, QueryEngine, MemoryLifecycle, EventStreamStore, EmbeddingService |
| `brain/` | 6 | Reasoner, DecisionEngine, StateModel, LearningFeedback |
| `perception/` | 7 | SensorFramework, SignalProcessing, PatternRecognition, SensorFusion |
| `execution/` | 8 | ActionPlanner, AgentCoordinator, ActionLibrary, ExecutionMonitor |
| `services/` | 4+ | Auxiliary platform services |

---

## Test Organization

### Unit Tests

Unit tests are co-located with the code they test, in a `#[cfg(test)] mod tests` block at the bottom of each source file or in a dedicated `tests.rs` submodule:

```
core/src/events/
  mod.rs          -- Module implementation
  tests.rs        -- Unit tests for the events module
```

### Integration Tests

Integration tests live in `tests/` at the crate root. Each file in `tests/` is compiled as a separate binary:

```
core/tests/
  event_bus_integration.rs
  lifecycle_integration.rs
  config_integration.rs

runtime/tests/
  scheduler_integration.rs
  supervisor_integration.rs
  permission_integration.rs
```

Workspace-level integration tests (testing cross-crate interactions) live in the root `tests/` directory:

```
tests/
  platform_integration.rs
```

### Test Conventions

- Test functions are named `test_<function>_<condition>`: `test_dispatch_event_success`, `test_dispatch_no_handler`.
- Integration tests use common setup helpers defined in a `tests/common/mod.rs` module.
- Async tests use `#[tokio::test]`.
- Property-based tests (using `proptest` or `quickcheck`) are suffixed with `_prop`: `test_scheduling_invariants_prop`.

---

## Benchmark Organization

Benchmarks are located in `benches/` at the crate root and use the Criterion framework:

```
runtime/benches/
  runtime_bench.rs   -- Scheduler and supervisor benchmarks
```

Run benchmarks with:

```bash
cargo bench --workspace
```

Individual benchmark functions are annotated with `criterion_group!` and `criterion_main!` macros.

---

## Docs Organization

The `docs/` directory contains:

| File/Dir | Content |
|---|---|
| `architecture.md` | System architecture description and layer diagrams |
| `principles.md` | The 12 design principles that guide all decisions |
| `glossary.md` | Standardized vocabulary for the project |
| `coding-standards.md` | Rust coding standards and conventions |
| `contributing.md` | Contribution workflow and expectations |
| `project-structure.md` | This file: directory and crate reference |
| `build.md` | Build system guide and troubleshooting |
| `development.md` | Development environment and workflow guide |
| `testing.md` | Testing philosophy, conventions, and tooling |
| `release.md` | Release process and versioning |
| `security.md` | Security model, audit, and vulnerability reporting |
| `deployment.md` | Deployment configuration and operations |
| `roadmap.md` | Phase roadmap and milestones |
| `vision.md` | Long-term project vision |
| `adr/` | Architecture Decision Records |
| `api/` | API reference documentation |
| `decisions/` | Design decision records |
| `diagrams/` | Architecture and flow diagrams |
| `modules/` | Per-module detailed documentation |

---

## Key Files

| File | Purpose |
|---|---|
| `Cargo.toml` | Workspace definition, shared package metadata |
| `Cargo.lock` | Reproducible dependency resolution (committed) |
| `scripts/setup.sh` | One-command development environment setup |
| `core/src/lib.rs` | Core crate root: module declarations and re-exports |
| `core/src/error.rs` | Core error type definition |
| `runtime/src/lib.rs` | Runtime crate root: module declarations and re-exports |
| `runtime/src/error.rs` | Runtime error type definition |
| `runtime/src/runtime.rs` | Runtime orchestrator initialization |
| `runtime/benches/runtime_bench.rs` | Criterion benchmark definitions |
| `docs/architecture.md` | System architecture reference |
| `docs/principles.md` | Design principles (must read before any code) |
| `docs/glossary.md` | Standardized terminology |

---

## Dependency Graph (Internal)

```
ai-os-core (core/)
     ^
     |
ai-os-runtime (runtime/)
```

The dependency graph is a DAG. `runtime` depends on `core`. Future crates (`system`, `memory`, `brain`, etc.) will each depend on the crate one phase below them, maintaining the clean architecture layering.
