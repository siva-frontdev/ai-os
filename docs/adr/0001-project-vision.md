# ADR-0001: Project Vision and Scope

## Status

Accepted

## Date

2025-01-15

## Context

The project aims to build an AI-native operating platform on top of Arch Linux. The term "AI-native" requires precise definition to bound scope, guide design decisions, and communicate intent to contributors and stakeholders.

The key questions to resolve are:

1. **What does "AI-native OS" mean for this project?** The phrase has multiple interpretations in the industry, ranging from an OS with built-in ML inference acceleration to a fully autonomous self-managing system. We must define our interpretation to align all contributors.

2. **What is in scope and what is out of scope?** Without clear boundaries, the project risks scope creep, especially in the AI domain where possibilities are endless. Each proposed feature must be filterable against the scope document.

3. **What is the long-term delivery roadmap?** A multi-year project needs a phased approach to demonstrate value early and manage complexity. Each phase must produce a working, testable increment.

4. **What architectural principles guide the design?** Foundational constraints (language, architecture style, module isolation) must be established before implementation begins to prevent costly rework.

5. **What is the target deployment model?** Should this be a replacement OS, a hypervisor layer, a userspace platform, or something else? The answer determines the entire architecture.

6. **Who are the target users?** AI researchers, application developers, system integrators, or end users? Each audience has different expectations for APIs, documentation, and deployment friction.

7. **How does this relate to existing AI platforms?** The landscape includes LangChain, AutoGPT, CrewAI, and various agent frameworks. We must differentiate and avoid duplicating functionality that exists in mature external projects.

### Landscape Analysis

The AI agent platform ecosystem in early 2025 is active but fragmented:

- **LangChain / LangGraph**: Provide agent orchestration as a Python library. They operate at the application level, not the OS level. Our platform builds on similar concepts but exposes them as OS-level services with lifecycle management, permissions, and resource control.
- **AutoGPT / AgentGPT**: Demonstrate the potential of autonomous AI agents but lack production-grade infrastructure (no permission model, no resource limits, no supervision). Our platform provides the missing infrastructure.
- **CrewAI**: Multi-agent orchestration as a Python framework. Useful for specific delegation patterns. Our platform could host CrewAI agents as a tool or extension.
- **OpenAI Assistants API**: Cloud-hosted agent platform. Our platform is self-hosted, open-source, and not tied to a single provider.
- **Dify / Flowise**: Low-code AI application builders. They target a different audience (application builders vs. infrastructure developers).

Our platform differentiates by focusing on the OS-level substrate: lifecycle management, permissions, resource control, and event-driven extensibility. We do not compete with application-level frameworks — we provide the platform they run on.

### Analysis of Deployment Targets

We evaluated the following deployment models:

**Standalone OS (custom Linux distribution or kernel module):** Rejected. Building a custom kernel or distribution is a massive undertaking. It would take years before any AI-specific value is delivered. Users would need to replace their existing OS, which is a high adoption barrier.

**Virtual machine monitor / hypervisor (Type 2):** Rejected. A hypervisor provides strong isolation but adds VM boot time (seconds to minutes), memory overhead (per-VM OS overhead), and management complexity. AI agents need fast startup (milliseconds, not seconds) and efficient resource sharing.

**Container runtime (Docker/Podman integration):** Considered as an isolation mechanism for tool execution (Phase 6). We may use containers for sandboxing code execution, but the platform itself is not a container orchestrator.

**Library / framework (pip install / cargo add):** Rejected as the primary model. A library cannot provide lifecycle management (no daemon process), resource enforcement (no cgroups access), or permission checking (no centralized policy). However, the platform will provide client SDKs for multiple languages in Phase 8.

**Userspace platform service (our choice):** The platform runs as a system daemon (similar to systemd or Docker daemon) on Arch Linux. It uses Linux kernel features (cgroups, namespaces, seccomp, epoll, io_uring) through safe Rust wrappers. This provides OS-level capabilities without kernel development.

### Interpretation of "AI-native"

We define "AI-native OS" as an operating platform where AI capabilities are first-class primitives, not bolted-on applications. This means:

- The platform provides built-in services for model loading, inference scheduling, and resource management for AI workloads. These services are available to all agents through the same mechanism — an event-driven service bus — rather than through ad-hoc library integrations.
- AI agents are first-class citizens with managed lifecycles, permissions, and communication channels, analogous to how processes are first-class citizens in traditional OSes. An agent can be created, started, paused, resumed, inspected, and terminated through a uniform set of system calls (events).
- The system exposes AI capabilities (prompting, embedding, code execution, tool use) as OS-level services rather than library calls. An agent requests inference through the EventBus, not by importing an LLM client library.
- The platform itself uses AI for introspection, optimization, and self-healing where appropriate, but AI-augmented platform services are layered on top of deterministic core infrastructure. The core platform (scheduling, event dispatch, resource accounting) never depends on AI for correct operation.
- The distinction between "platform" and "agent" is explicit. The platform provides services; agents consume them. The platform does not run its own agents for self-management in the initial phases.

### Scope Boundaries

**In scope:**

- A runtime platform for managing AI agents as managed processes with lifecycle, permissions, and resource limits.
- Built-in AI services: LLM inference orchestration across multiple providers, embedding generation, code execution sandboxes, tool execution with sandboxing.
- An event-driven module system that allows first-party and third-party modules to extend platform capabilities. Modules communicate via a typed EventBus with discovery and subscription.
- Session management for persistent, stateful AI interactions with context window management and history persistence.
- A permission and security model for AI agents: what an agent can access, what tools it can invoke, what system resources it can consume, with role-based access control.
- Async-first, high-performance infrastructure in Rust leveraging Tokio for all I/O and inter-module communication.
- Deployment as a userspace platform on Arch Linux (not a standalone OS kernel), installable via packages and controllable via systemd.
- Observability infrastructure: structured logging, distributed tracing across event chains, metrics collection for all subsystems.
- A plugin system (Phase 9) for loading third-party modules implementing the module trait interfaces.

**Out of scope:**

- Developing our own LLM models, training infrastructure, or fine-tuning pipelines. We consume models through standardized inference APIs.
- Kernel development or OS kernel modifications. We run entirely in userspace.
- Replacing the Linux kernel or Arch Linux userland. The platform is an application on top of Linux, not a replacement for it.
- General-purpose application hosting. This is not a replacement for Docker, Podman, or Kubernetes. We host AI agents, not arbitrary applications.
- Hardware design or AI accelerator chip development. We interface with existing hardware through standard APIs (CUDA, ROCm, Vulkan).
- Building a general AI framework or library. We use existing libraries (candle, burn, llama.cpp bindings) rather than building our own.
- Real-time or hard-deadline guarantees. The platform is a best-effort userspace system.
- Mobile or embedded deployment. Target is workstation and server Linux environments.

### Roadmap Overview

The project is organized into nine phases, each with a clear deliverable:

**Phase 1 — Dev Env (estimated 4 weeks):** Establish the Cargo workspace structure, CI/CD pipeline with GitHub Actions, code quality tooling (Clippy, rustfmt, audit), crate scaffolding with `core/` and `runtime/` directories, README and contributing guide, license selection (MIT or Apache 2.0), and developer documentation.

**Phase 2 — Core (estimated 6 weeks):** Implement the `core/` crate containing foundational types: `AgentId`, `EventId`, `TraceId`, `SessionId`, errors (`CoreError`), the `Event` and `EventBus` traits, the `InMemoryEventBus` implementation, configuration trait and `Config` loader, storage abstraction traits, and domain event types that all higher layers will use.

**Phase 3 — Runtime (estimated 8 weeks):** Implement the `runtime/` crate with eight subsystems: Scheduler (multi-level feedback queue), Supervisor (health monitoring and restart policies), SessionManager (session CRUD and state), TaskManager (task queues and dispatch), ContextManager (context propagation and window management), StateMachine (agent state transitions), ResourceManager (CPU/memory/GPU tracking), PermissionChecker (RBAC). Each subsystem is a trait with a default implementation.

**Phase 4 — System (estimated 6 weeks):** Integrate the eight runtime subsystems into a coherent `Runtime` struct. Implement the state machine transition matrix, permission enforcement in all subsystems, system health checks, observability (tracing spans on all EventBus dispatches), metrics export (Prometheus-compatible), and integration tests for cross-subsystem scenarios.

**Phase 5 — AI Engine (estimated 8 weeks):** Implement the `ai-engine/` crate with model abstraction layer, LLM provider adapters (OpenAI-compatible, Anthropic, local models via llama.cpp), embedding service, streaming response handling, prompt templating, and model lifecycle management (load, unload, warm, cold).

**Phase 6 — Tools (estimated 6 weeks):** Implement the `tools/` crate with tool registry, tool execution sandbox (WebAssembly or container-based), filesystem tool with path sandboxing, web search tool, code execution tool (sandboxed Python/Rust), and tool permission system integrated with PermissionChecker.

**Phase 7 — Session (estimated 6 weeks):** Implement the `session/` crate with full session lifecycle, conversation history persistence (SQLite/PostgreSQL behind storage trait), context window sliding and summarization, session continuity across platform restarts, and session export/import.

**Phase 8 — API Layer (estimated 8 weeks):** Implement the `api/` crate with gRPC service definitions (protobuf), HTTP/REST API (Axum), WebSocket for streaming responses, CLI application (Clap), SDK stubs for Python and TypeScript, and authentication/authorization at the API boundary.

**Phase 9 — Polish (estimated 8 weeks):** Performance profiling and optimization, security audit, documentation completion (API reference, architecture guide, operator guide), packaging (Arch Linux PKGBUILD, Docker image, deb/rpm), distribution setup, and community contribution guidelines.

### Alternatives Considered for Deployment Model

**Standalone OS (custom kernel):** Rejected. Building and maintaining a custom kernel is an enormous undertaking that would consume years before delivering any AI-specific value. Userspace deployment allows us to leverage the existing Linux ecosystem (drivers, filesystems, networking) and focus on our differentiator: AI agent management.

**Virtual machine monitor / hypervisor:** Rejected. A Type-2 hypervisor would provide strong isolation but adds significant overhead (VM boot times, memory overhead) for AI agents that need fast startup and tight resource coupling. We use Linux cgroups and namespaces for isolation instead.

**Container orchestrator (Kubernetes operator):** Considered but rejected as the primary model. Kubernetes is designed for stateless microservices, not stateful AI agents with long-running sessions. However, Phase 8 may provide a Kubernetes integration for deploying the platform itself on cluster infrastructure.

### Architectural Principles

1. **Clean Architecture**: Outer layers depend on inner layers. Core domain logic has no framework dependencies. See [ADR-0002](./0002-clean-architecture.md).
2. **Event-Driven**: Modules communicate through events, not direct calls. See [ADR-0004](./0004-event-driven.md).
3. **Async-First**: All I/O and inter-module communication is asynchronous using Tokio.
4. **Rust by Default**: Systems-level code in Rust; scripting and extensibility through WebAssembly in later phases.
5. **Fail Closed**: Security and permission decisions default to denial. Explicit allow-list for every agent capability.
6. **Observability by Design**: Every module emits structured telemetry via the `tracing` crate. No module is added without observability.
7. **Incremental Delivery**: Each phase produces a working, testable increment that can be demonstrated and integrated.

## Decision

We adopt the following foundational commitments:

1. **Definition**: "AI-native OS" means a userspace operating platform where AI agents and AI services are first-class managed primitives, deployed on Arch Linux, written in Rust, with AI capabilities exposed as OS-level services through an event-driven architecture.

2. **Nine-phase roadmap**: The project will deliver incrementally across nine phases, each building on the previous. Phase boundaries are defined by integration milestones, not arbitrary dates. Each phase has a defined scope and acceptance criteria documented in the project roadmap.

3. **Clean layered architecture**: The codebase follows clean architecture principles with strict dependency direction. See [ADR-0002](./0002-clean-architecture.md).

4. **Rust as primary language**: All platform code is written in Rust. See [ADR-0003](./0003-rust.md).

5. **Event-driven communication**: Inter-module communication uses an EventBus pattern. See [ADR-0004](./0004-event-driven.md).

6. **Async-first runtime**: All asynchronous operations use the Tokio runtime. Blocking operations are isolated in dedicated thread pools via `tokio::task::spawn_blocking`.

7. **Deployment model**: The platform runs as a userspace service on Arch Linux. It does not replace or modify the host OS kernel. Isolation is provided by Linux cgroups, namespaces, and seccomp.

8. **Scope boundary enforcement**: Any feature that falls outside the defined scope requires a new ADR to amend this document. The ADR must justify why the scope should be expanded.

## Consequences

### Positive

- **Clear scope boundaries** prevent feature creep and keep the project focused on its core value proposition. Every proposed feature can be evaluated against the in-scope / out-of-scope list.
- **Phased delivery** provides regular integration milestones and demonstrable progress. Stakeholders see working software every 4-8 weeks.
- **Architectural principles** established upfront reduce the risk of costly re-architecture later. Decisions made in Phase 1 are still valid in Phase 9.
- **Rust language choice** provides memory safety, performance, and a strong type system for modeling complex AI domain concepts. The ownership model maps naturally to agent lifecycle management.
- **Userspace deployment** avoids the complexity of kernel development while still delivering OS-level abstractions for AI agents. We stand on the shoulders of the Linux kernel.
- **Event-driven architecture** enables loose coupling between modules. Individual modules can be developed, tested, and deployed independently.
- **Explicit scope boundaries** reduce decision fatigue. When a new idea arises, the team can quickly determine whether it belongs in the platform or in an external integration.

### Negative

- **Userspace limitations** mean we cannot provide hard real-time guarantees or modify kernel scheduling for AI workloads. Low-latency inference scheduling depends on kernel preemption settings beyond our control.
- **Nine-phase roadmap** is a long delivery timeline. Value is not fully realized until later phases. Early adopters get infrastructure without the AI services that make it valuable.
- **Rust learning curve** may slow initial development and limit the contributor pool. We invest in documentation and mentoring to mitigate this.
- **"AI-native" definition** may differ from community expectations. Some expect a standalone OS, others expect a full AI framework. Ongoing communication is needed to manage perception.
- **Scope rigidity** may cause us to miss valuable adjacent opportunities. The ADR amendment process provides an escape valve, but it adds friction.
- **Reliance on Arch Linux** ties the platform to a specific distribution. While Arch is our primary target, the platform should be portable to other Linux distributions. Phase 9 may add additional distribution packaging.

## Compliance

1. All new modules must be placed in the appropriate phase directory under the Cargo workspace, as defined in the workspace `Cargo.toml`.
2. No module in a higher-numbered phase may depend on a module in a later phase. CI enforces this with a custom dependency checker.
3. The README at the repository root must document the current phase and link to the roadmap document.
4. Architectural principle violations must be flagged in code review and either fixed or escalated with a new ADR.
5. CI enforces that `core/` has zero dependencies on `runtime/` or higher layers. This is checked by `cargo metadata` graph analysis.
6. The scope boundary document (this ADR) must be reviewed and reaffirmed at the start of each phase.
7. Any feature that crosses the scope boundary must have a corresponding ADR before implementation begins.
8. Each phase must have a written acceptance criteria document. A phase is not considered complete until all criteria are met.

## Notes

- This ADR was reviewed and accepted during the Phase 1 kickoff meeting on 2025-01-15 with unanimous consent.
- The nine-phase roadmap was reduced from an initial twelve-phase proposal; phases 8 and 9 were merged during review. An original Phase 10 (Ecosystem) was deferred indefinitely.
- The definition of "AI-native" will be revisited at the end of Phase 5 (AI Engine) to validate assumptions against real-world usage. If the definition needs adjustment, an amending ADR will be proposed.
- During Phase 2 planning, the team debated whether to include the AI Engine in scope. The decision was reaffirmed: AI Engine is Phase 5, not Phase 2.
- The term "operating platform" was chosen over "operating system" to emphasize that this runs on top of Linux, not as a replacement.

## References

- [ADR-0002: Clean Architecture with Layered Modules](./0002-clean-architecture.md)
- [ADR-0003: Rust as Implementation Language](./0003-rust.md)
- [ADR-0004: Event-Driven Architecture via EventBus](./0004-event-driven.md)
- [ADR-0005: Runtime Platform Design](./0005-runtime-platform.md)
- [Project Roadmap](../roadmap.md)
- [Contributing Guide](../CONTRIBUTING.md)
- [Architecture Overview](../architecture.md)
