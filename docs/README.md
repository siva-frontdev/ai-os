# AI-native OS Documentation

## Project Overview

AI-native OS is an open-source operating platform built on Arch Linux that reimagines the relationship between the operating system and artificial intelligence. Rather than treating AI as an application layer bolted onto a conventional OS, AI-native OS embeds intelligence as a first-class citizen throughout the system stack. The platform is written entirely in Rust, following clean layered architecture principles, event-driven design, and an async-first execution model powered by Tokio.

The project spans nine development phases, from foundational environment setup through full intelligence integration. Phases 1 through 3 (Dev Environment, Core Platform, Runtime Platform) are complete. Phase 4 (System Platform) is in active development. Phases 5 through 9 (Memory, Brain, Perception, Execution, Intelligence Integration) are planned on a multi-year roadmap.

AI-native OS is designed to be modular, observable, secure by default, and extensible through a plugin architecture. All intra-system communication flows through an EventBus, enabling loose coupling between components and comprehensive observability via distributed tracing.

### Core Tenets

- **Intelligence is a platform primitive**, not an application-layer concern. Memory, perception, reasoning, and execution are first-class abstractions provided by the operating platform.
- **Clean layers with strict dependency direction** ensure that the system remains comprehensible and maintainable as it grows across nine phases of development.
- **Events are the universal communication medium.** Every component communicates through typed, traceable events dispatched on the EventBus. No component holds a direct reference to another.
- **Security is designed in from the start.** Every interaction is authenticated, authorized, and audited. The default configuration is secure; permissions are denied by default.
- **The system observes itself.** Comprehensive observability (logs, metrics, traces, events) is built into every component. The observability subsystem is itself observable.

### Current Status

| Aspect | Status |
|---|---|
| Phase 1 (Dev Environment) | Completed |
| Phase 2 (Core Platform) | Completed |
| Phase 3 (Runtime Platform) | Completed |
| Phase 4 (System Platform) | In Progress |
| Phases 5-9 (Intelligence) | Planned |
| Rust toolchain | Stable, latest |
| Async runtime | Tokio (multi-threaded) |
| Build system | Cargo workspace |

---

## Quick Links

| Document | Description |
|---|---|
| [Vision](vision.md) | Project philosophy, long-term goals, what success looks like |
| [Architecture](architecture.md) | Layered clean architecture, event-driven design, module dependency graph |
| [Principles](principles.md) | Twelve design principles with explanations and examples |
| [Roadmap](roadmap.md) | Complete phase-by-phase development roadmap with timeline |
| [Glossary](glossary.md) | Comprehensive terminology reference (40+ terms) |

---

## Getting Started

### Prerequisites

- **Arch Linux** (host or containerized). The platform is developed and tested on Arch Linux. Other Linux distributions may work but are not officially supported.
- **Rust toolchain** (stable, latest). Install via `rustup` if not already present: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
- **Tokio runtime** (included as a Cargo dependency; no separate installation required).
- **Git** for source code management.
- **Build dependencies**: `base-devel` group (Arch Linux) or equivalent build tools for your distribution.

### Building from Source

```bash
git clone https://github.com/ai-os/ai-os
cd ai-os
cargo build --release
```

The first build will download and compile all dependencies. Subsequent builds use the cached build artifacts. The `--release` flag enables optimizations; omit it for faster compilation during development.

### Running the Platform

```bash
cargo run --bin ai-os
```

This starts the platform with default configuration. The platform initializes the Core Platform services, registers the Runtime Platform modules, and begins processing events. Console output shows the service lifecycle events as each subsystem starts.

### Running Tests

```bash
cargo test --all
```

This runs the full test suite across all workspace crates. Tests include unit tests (individual functions and methods), integration tests (service interactions through the EventBus), and property-based tests (invariants that must hold for all inputs).

### Checking Linting

```bash
cargo clippy --all-targets --all-features
```

The project uses Clippy with a project-specific lint configuration. All Clippy warnings must be resolved before pull requests are merged. The CI pipeline enforces this.

### Running a Subset of Tests

To run tests for a specific crate:

```bash
cargo test -p ai-os-core
cargo test -p ai-os-runtime
```

To run tests matching a specific name pattern:

```bash
cargo test --all -- health
```

### Exploring the Codebase

The repository is organized by crate, mirroring the layered architecture:

```
ai-os/
  core/              # Core Platform (Phase 2)
  runtime/           # Runtime Platform (Phase 3)
  system/            # System Platform (Phase 4 - in development)
  memory/            # Memory Platform (Phase 5 - planned)
  brain/             # Brain Platform (Phase 6 - planned)
  perception/        # Perception Platform (Phase 7 - planned)
  execution/         # Execution Platform (Phase 8 - planned)
  integration/       # Intelligence Integration (Phase 9 - planned)
  docs/              # Documentation (this directory)
```

Each crate contains a `src/` directory with the module source code, a `tests/` directory with integration tests, and a `Cargo.toml` manifest. The workspace root `Cargo.toml` defines shared dependency versions and workspace membership.

### Key Cargo Commands

| Command | Purpose |
|---|---|
| `cargo build` | Compile the workspace (debug mode) |
| `cargo build --release` | Compile with optimizations |
| `cargo test --all` | Run all tests across workspace |
| `cargo clippy --all-targets` | Run lint checks |
| `cargo fmt --all` | Format code according to project style |
| `cargo doc --open` | Build and open Rustdoc documentation |
| `cargo audit` | Check for known security vulnerabilities in dependencies |

---

## Documentation Structure

The documentation is organized into six root-level files, each serving a distinct purpose within the project's information architecture. They are designed to be read both sequentially (as a learning path) and independently (as reference material).

### Conceptual Documentation

- **[vision.md](vision.md)** — Defines the "why" of the project. Read this first to understand the motivation, philosophy, and long-term aspirations that guide every technical decision. The vision document answers: What problem does this project solve? Why does it exist? What does success look like on a 10-year horizon?

- **[principles.md](principles.md)** — Defines the "how" of the project. These twelve principles are the decision-making framework used by every contributor. When a design choice is unclear, the principles resolve it. Principles include Clean Architecture, Event-Driven Design, Fail Fast and Gracefully, Security by Default, and eight more.

### Technical Documentation

- **[architecture.md](architecture.md)** — Defines the "what" of the system. This document describes the layered clean architecture, component responsibilities, communication patterns, and cross-cutting concerns. Essential reading before contributing code. Includes a module dependency graph, event flow descriptions, service lifecycle state machine, and technology stack decisions.

- **[roadmap.md](roadmap.md)** — Defines the "when" of the project. Tracks development progress across all nine phases with objectives, deliverables, dependencies, and effort estimates. Includes a visual timeline, risk assessment, and milestone targets.

### Reference Documentation

- **[glossary.md](glossary.md)** — Defines the terminology used throughout the project. Standardized vocabulary ensures that design discussions, code reviews, and documentation remain consistent. Refer to this when encountering unfamiliar terms. Contains 40+ entries with definitions and cross-references.

### How to Use These Documents

The recommended learning path for new contributors:

1. Start with [Vision](vision.md) (15 minutes) to understand project motivation and philosophy.
2. Read [Principles](principles.md) (20 minutes) to understand the design philosophy and decision framework.
3. Study [Architecture](architecture.md) (30 minutes) to understand system structure, communication patterns, and component responsibilities.
4. Review [Roadmap](roadmap.md) (15 minutes) to see current development status and upcoming work.
5. Refer to [Glossary](glossary.md) as needed (5 minutes per lookup) for term definitions when reading other documents.
6. Return to this [README](README.md) as the navigation hub for finding specific information.

### Document Map

```
                      +------------------+
                      |    README.md      |  (Navigation hub)
                      +------------------+
                              |
            +-----------------+------------------+
            |                 |                  |
            v                 v                  v
    +--------------+  +--------------+  +------------------+
    |  vision.md   |  | principles.md|  | architecture.md  |
    |  (Why)       |  |  (How)       |  |  (What)          |
    +--------------+  +--------------+  +------------------+
                                                  |
            +-----------------+------------------+
            |                 |
            v                 v
    +--------------+  +--------------+
    |  roadmap.md  |  |  glossary.md |
    |  (When)      |  |  (Terms)     |
    +--------------+  +--------------+
```

### File Conventions

- All documents are written in Markdown (CommonMark specification).
- Documents use ATX-style headings with a single space after the `#`.
- Code blocks specify a language tag for syntax highlighting.
- Tables are used for structured data.
- Cross-references use relative paths: `[Architecture](architecture.md)`.
- Terms defined in the glossary are linked on first use in each document.
- Acronyms are defined on first use per document.

---

## How to Contribute to Docs

### Documentation Standards

Documentation is treated as a first-class deliverable in AI-native OS. A feature is not complete until its documentation is complete. Documentation follows the same review process as code changes.

- All documentation is written in Markdown following the CommonMark specification.
- Use relative paths for cross-document links (e.g., `[Architecture](architecture.md)`).
- Lines should be wrapped at 100 characters where practical to improve readability in terminal-based editors.
- Use ATX-style headings (`#`, `##`, `###`) with a single space after the `#` character. Do not use underlined headings.
- Tables should be used for structured data. Keep tables readable in source form by aligning column formatting.
- Code blocks must specify a language tag (e.g., `rust`, `bash`, `text`, `toml`). Use fenced code blocks with triple backticks.
- Terms defined in the [Glossary](glossary.md) should be linked on first use in each document.
- Avoid inline HTML. Markdown should be sufficient for all formatting needs.
- Do not use emojis.
- Use precise technical terminology as defined in the glossary.

### Contribution Workflow

1. **Identify** gaps, errors, or improvements in the existing documentation. Common issues include: outdated descriptions, missing cross-references, unclear explanations, and incomplete examples.
2. **Open an issue** describing the proposed change if it is non-trivial. This allows discussion before implementation.
3. **Submit changes** via pull request to the repository. Documentation changes follow the same PR process as code changes.
4. **Ensure cross-references remain valid.** If you rename a section or document, update all links pointing to it. Broken links are a bug.
5. **Keep language precise, professional, and free of marketing hyperbole.** Documentation describes what the system does, not what it promises to do.
6. **Maintain documentation alongside code changes.** A pull request that adds a new event type must include documentation of that event type.

### Style Guide

- Use **active voice** where possible ("the EventBus dispatches events" not "events are dispatched by the EventBus").
- Prefer **present tense** ("the system routes messages" not "the system will route messages").
- Use second person ("you") for instructional content sparingly; prefer third person for reference content.
- Define acronyms on first use per document (e.g., "Asynchronous Service Supervisor (ASS)").
- Use **serial commas** (Oxford commas) in lists of three or more items.
- Use bold for emphasis, not italics. Reserve italics for foreign terms and titles.
- Use code font for type names, function names, file paths, and command-line examples.
- Avoid colloquialisms, idioms, and informal language.
- Use precise technical terminology defined in the glossary. If a term is not in the glossary, consider adding it.

### Review Criteria

Documentation pull requests are reviewed against these criteria:

| Criterion | Question |
|---|---|
| Accuracy | Does the content correctly describe the system as implemented? |
| Clarity | Is the content understandable to the target audience? |
| Completeness | Are there gaps that need filling or edge cases unaddressed? |
| Consistency | Does the content align with the project's terminology, style, and conventions? |
| Cross-referencing | Are related documents properly linked? Are links valid? |
| Timeliness | Is the content current with the codebase? Outdated documentation is a bug. |

### Types of Documentation Contributions

| Type | Description | Effort |
|---|---|---|
| Bug fix | Correcting an error in existing documentation | Low |
| Clarification | Rewording unclear passages | Low |
| Gap filling | Adding missing content (new sections, examples) | Medium |
| Cross-reference | Adding links between related documents | Low |
| New document | Creating a new documentation file | High |
| Translation | Translating documentation to another language | High |

---

## Learning Path

The following learning path is designed for new contributors who want to understand the project deeply.

### Week 1: Orientation

1. Read [Vision](vision.md) (30 min).
2. Read [Principles](principles.md) (30 min).
3. Read [Architecture](architecture.md) (45 min).
4. Read [Roadmap](roadmap.md) (20 min).
5. Skim [Glossary](glossary.md) (15 min).
6. Clone the repository and build from source (30 min).
7. Run the test suite and explore test structure (30 min).

### Week 2: Core Platform

1. Read the Core Platform `src/` directory structure.
2. Study the EventBus implementation (event routing, subscriptions, middleware).
3. Study the Service Lifecycle implementation (state machine, transitions).
4. Study the Logger and HealthMonitor implementations.
5. Run Core Platform tests with `cargo test -p ai-os-core`.
6. Attempt to fix a documentation issue or add a test.

### Week 3: Runtime Platform

1. Read the Runtime Platform `src/` directory structure.
2. Study the Scheduler, Supervisor, and Task Manager.
3. Study the Context Manager (trace/span propagation).
4. Study the Permission Checker.
5. Run Runtime Platform tests with `cargo test -p ai-os-runtime`.
6. Write a small integration test that exercises an EventBus flow across Core and Runtime services.

### Week 4: Contribution

1. Identify a good first issue from the issue tracker.
2. Discuss the approach with maintainers.
3. Implement the change and write tests.
4. Update documentation for the change.
5. Submit a pull request.

---

## Technology Stack

| Component | Choice | Rationale |
|---|---|---|
| Language | Rust (stable, latest) | Memory safety without GC, zero-cost abstractions, async support, strong type system |
| Async runtime | Tokio | Production maturity, work-stealing scheduler, extensive ecosystem |
| Serialization | Serde | Standard Rust serialization framework, derive macros, format-agnostic |
| Event Bus | Custom (in-house) | Designed for typed, traceable, async event routing with middleware pipeline |
| Tracing | tracing crate + OpenTelemetry | Industry standard for distributed tracing, async-aware, structured spans |
| Metrics | metrics crate | Lightweight, composable, multiple exporter support |
| CLI | clap | Derive-based argument parsing, comprehensive help generation |
| Configuration | config crate | Layered configuration with file/env/CLI merge, hot-reload support |
| Container | Docker / Podman API | Industry standard container runtimes, stable API |
| CI/CD | GitHub Actions | Tight GitHub integration, matrix builds, caching |

---

## Related Resources

- **Source Repository**: [github.com/ai-os/ai-os](https://github.com/ai-os/ai-os)
- **Rustdoc**: Generated API documentation published per release. Build locally with `cargo doc --open`.
- **Issue Tracker**: [github.com/ai-os/ai-os/issues](https://github.com/ai-os/ai-os/issues) — Bug reports, feature requests, and tasks.
- **Discussion Forum**: [github.com/ai-os/ai-os/discussions](https://github.com/ai-os/ai-os/discussions) — Questions, ideas, and community conversation.
- **Continuous Integration**: [github.com/ai-os/ai-os/actions](https://github.com/ai-os/ai-os/actions) — Build and test status.

---

## FAQ

### Is AI-native OS a Linux distribution?

No. AI-native OS runs on top of Arch Linux. It is an operating platform that manages services, resources, and intelligence, not a standalone operating system kernel. It leverages the Linux kernel for hardware abstraction and process management while adding AI-native abstractions at the platform layer.

### Can I run AI-native OS on a non-Arch distribution?

AI-native OS is developed and tested on Arch Linux. Other Linux distributions may work, particularly those with recent kernel versions and systemd. Official support is limited to Arch Linux. Community contributions for additional platform support are welcome.

### Does AI-native OS require special hardware?

No special hardware is required. AI-native OS runs on standard x86-64 hardware. GPU acceleration for machine learning workloads (Phases 6-9) will benefit from NVIDIA or AMD GPUs but is not required.

### Is AI-native OS production-ready?

Not yet. Phases 1-3 (Core and Runtime Platform) are complete and tested, but the system is not recommended for production use until Phase 4 (System Platform) is complete and the platform has undergone security hardening.

### How is AI-native OS different from Kubernetes?

Kubernetes is an orchestration platform for containerized workloads. AI-native OS is an operating platform that provides intelligence as a first-class system primitive. They operate at different layers of the stack. AI-native OS could potentially run on Kubernetes-managed infrastructure.

### Can I use individual AI-native OS components in my project?

Yes. Each crate in the workspace is a standalone Rust library. The EventBus, Logger, HealthMonitor, and other components can be used independently. However, the full value of the platform emerges when components are used together within the architectural framework.

---

## License

AI-native OS is open source software. Refer to the LICENSE file in the repository root for licensing details.

---

## Communication

- **GitHub Issues**: Bug reports and feature requests.
- **GitHub Discussions**: Questions, ideas, and community conversation.
- **Pull Requests**: Code and documentation contributions.
- **Project Board**: [github.com/ai-os/ai-os/projects](https://github.com/ai-os/ai-os/projects) — Development tracking.

---

## Versioning and Stability

AI-native OS follows Semantic Versioning (SemVer 2.0.0). Major version zero (0.x.x) indicates that the API is not stable and may change at any time. Once the project reaches 1.0.0 (targeted at the completion of Phase 4), public APIs will follow stable deprecation policies.

Components within each phase may have different stability levels:
- Core Platform (Phase 2): API is approaching stability.
- Runtime Platform (Phase 3): API is partially stable; breaking changes are infrequent but possible.
- System Platform (Phase 4): API is actively evolving; breaking changes are expected.

---

*This documentation is maintained by the AI-native OS team. For questions or corrections, open an issue or submit a pull request.*
