# Glossary

## Introduction

This glossary defines the standardized vocabulary used throughout the AI-native OS project. Consistent terminology is essential for clear communication in design documents, code reviews, issue tracking, and code comments. Terms defined here are used precisely; if a term is used in a way that deviates from this glossary, that is a documentation or code bug.

Terms are listed alphabetically. Each entry includes the term, a brief definition, and related terms for cross-reference.

---

## A

### AI-OS

The AI-native Operating System project. An operating platform built on Arch Linux that embeds artificial intelligence as a first-class citizen throughout the system stack, rather than treating it as an application-layer concern. The project spans nine development phases and is written entirely in Rust.

**Related terms**: Agent, Brain, Core, Platform, System Platform

### Agent

An autonomous or semi-autonomous software entity that operates within the AI-native OS framework. Agents perceive their environment through the Perception Platform, reason using the Brain Platform, and act through the Execution Platform. Agents may represent users, services, or automated system management functions.

**Related terms**: Brain, Execution Platform, Perception Platform, Worker

---

## B

### Brain

The core reasoning and decision-making subsystem of AI-native OS (Phase 6). The Brain Platform maintains system state models, performs inference on events, executes reasoning operations, and drives the learning feedback loop. It consumes input from the Perception Platform, stores learned patterns in the Memory Platform, and directs the Execution Platform.

**Related terms**: Execution Platform, Intelligence Integration, Memory Platform, Perception Platform

### Bus

See EventBus.

---

## C

### Capability

A named, versioned, and access-controlled function or resource exposed by a module. Capabilities are registered with the Capability Discovery system (Phase 4) and are subject to permission checks before use. Each capability has a unique identifier, input/output types, and a security classification.

**Related terms**: Capability Discovery, Permission Checker, Policy, Provider

### Capability Discovery

A System Platform (Phase 4) subsystem that maintains a registry of all capabilities exposed by modules. Provides query, subscription, and version negotiation interfaces. Enables modules to discover what other modules can do without direct coupling.

**Related terms**: Capability, Module, Plugin

### Clean Architecture

An architectural pattern that organizes software into concentric layers with strict dependency direction. Inner layers define interfaces and business rules; outer layers implement infrastructure details. Dependencies always point inward. AI-native OS implements Clean Architecture with layers corresponding to development phases.

**Related terms**: Architecture, Domain, Module, Phase

### Container

A Core Platform (Phase 2) module that provides an abstraction over container runtimes (Docker, Podman). Manages container lifecycle, resource limits, networking, and volume mounts. Used to isolate services and workloads within the platform.

**Related terms**: Core, Daemon, Plugin, Worker

### Context

The propagation of trace and span identifiers through asynchronous event chains. Context is carried by every event in the system, enabling distributed tracing across module boundaries. The Context Manager (Phase 3) handles context creation, propagation, and correlation.

**Related terms**: Context Manager, Span ID, Trace ID

### Context Manager

A Runtime Platform (Phase 3) subsystem responsible for generating, propagating, and correlating distributed tracing context. Each event receives a trace ID and span ID. The Context Manager ensures that context flows correctly across async boundaries and module boundaries.

**Related terms**: Context, Span ID, Trace ID

### Core

The foundational layer of AI-native OS (Phase 2). Provides the EventBus, Service Lifecycle, Logger, HealthMonitor, and Container modules. The Core Platform has no internal dependencies beyond the Rust standard library and external crates.

**Related terms**: Bus, Container, Health Monitor, Lifecycle, Logger

---

## D

### Daemon

A long-running background process managed by the Daemon Manager (Phase 4). Daemons perform continuous or periodic system functions such as monitoring, policy enforcement, and resource management. They follow the standard service lifecycle and communicate via the EventBus.

**Related terms**: Daemon Manager, Service, Supervisor, Worker

### Daemon Manager

A System Platform (Phase 4) subsystem responsible for starting, stopping, monitoring, and ordering daemon processes. Handles dependency resolution, restart policies, and health monitoring for all system daemons.

**Related terms**: Daemon, Service Lifecycle, Supervisor

### Domain

A bounded context within the system architecture representing a specific area of responsibility. Examples include the scheduling domain, security domain, and storage domain. Domains correspond to architectural layers and module boundaries.

**Related terms**: Architecture, Clean Architecture, Module

---

## E

### Event

A structured, typed, serializable message dispatched on the EventBus. Events are the universal communication medium in AI-native OS. Every event carries an ID, trace context, source identifier, type, payload, timestamp, priority, and security context.

**Related terms**: Bus, EventBus, Handler, Message

### EventBus

The central communication backbone of AI-native OS (Core Platform, Phase 2). The EventBus routes typed events from publishers to subscribers through a configurable middleware pipeline. It provides loose coupling, observability, security enforcement, and asynchronous dispatch. All inter-module communication flows through the EventBus.

**Related terms**: Bus, Event, Handler, Message, Subscription

### Execution Platform

The subsystem (Phase 8) responsible for autonomous and semi-autonomous task execution. Includes the Action Planner, Agent Coordinator, Action Library, Execution Monitor, and Human-in-the-Loop interface. The Execution Platform carries out decisions made by the Brain Platform.

**Related terms**: Agent, Brain, Intelligence Integration, Task

---

## H

### Handler

A function or closure that processes events of a specific type. Handlers are registered with the EventBus during service initialization. They receive deserialized event payloads, perform processing, and may dispatch response events. Handlers run as Tokio tasks.

**Related terms**: Event, EventBus, Service, Subscription

### Health Monitor

A Core Platform (Phase 2) subsystem that tracks the health status of all services in the system. Provides liveness checks (is the service running?) and readiness checks (is the service able to process work?). Health status is exposed via events and queryable endpoints.

**Related terms**: Core, Health Monitor, Supervisor

---

## I

### Intelligence Integration

The final architectural layer (Phase 9) that unifies Memory, Brain, Perception, and Execution into a cohesive intelligence subsystem. Provides self-optimization, self-healing, self-configuration, and cross-subsystem learning. Represents the culmination of the AI-native OS vision.

**Related terms**: Brain, Execution Platform, Memory Platform, Perception Platform

---

## L

### Lifecycle

See Service Lifecycle.

### Logger

A Core Platform (Phase 2) subsystem that provides structured, level-based logging throughout the system. Log entries include timestamps, module identifiers, trace context, severity levels, and structured fields. The Logger supports multiple output sinks (stdout, files, network) and integrates with the tracing infrastructure.

**Related terms**: Core, Event, Observability, Trace ID

---

## M

### Memory Platform

The subsystem (Phase 5) responsible for persistent, queryable storage of learned patterns, system state history, and event streams. Includes the Persistent Store, Query Engine, Memory Lifecycle Manager, Event Stream Store, and Embedding Service. Provides the long-term memory that enables the Brain Platform to learn and reason.

**Related terms**: Brain, Event, Intelligence Integration, Perception Platform

### Message

A generic term for data transmitted between components. In AI-native OS, all messages are Events dispatched on the EventBus. The term "message" is used in documentation when the specific event structure is not relevant to the discussion.

**Related terms**: Event, EventBus

### Module

A self-contained unit of functionality within the system. Modules correspond to Cargo crates and contain one or more services. Each module has a defined interface (events it publishes and subscribes to), a module ID, and a set of registered capabilities.

**Related terms**: Capability, Plugin, Service

---

## P

### Perception Platform

The subsystem (Phase 7) responsible for sensing and interpreting the environment. Includes the Sensor Framework, Signal Processing pipelines, Pattern Recognition services, Sensor Fusion, and Perception Event Stream. Provides processed, structured input to the Brain Platform.

**Related terms**: Brain, Intelligence Integration, Memory Platform

### Phase

A major development stage in the AI-native OS roadmap, corresponding to a layer in the clean architecture. There are nine phases: Dev Environment (1), Core Platform (2), Runtime Platform (3), System Platform (4), Memory Platform (5), Brain Platform (6), Perception Platform (7), Execution Platform (8), and Intelligence Integration (9).

**Related terms**: Architecture, Clean Architecture, Roadmap

### Plugin

A dynamically loadable (or separately process) module that extends the platform's capabilities. Plugins are managed by the Plugin System (Phase 4) and communicate with the platform exclusively through the EventBus. Plugins run in isolated processes to maintain memory safety.

**Related terms**: Capability, Module, Plugin

### Policy

A declarative rule that governs system behavior, access control, or resource allocation. Policies are defined in configuration files and evaluated by the Policy Engine (Phase 4). Policies can be updated at runtime via configuration events.

**Related terms**: Capability, Permission Checker, Policy, Security Context

### Policy Engine

A System Platform (Phase 4) subsystem that evaluates policies against events and actions. The Policy Engine determines whether a given action is permitted, required, or forbidden based on the current policy set and context.

**Related terms**: Permission Checker, Policy, Security Context

### Provider

An implementation of a capability interface. Providers are registered with the Capability Discovery system and are responsible for executing the actual work behind a capability. Multiple providers may implement the same capability interface, enabling provider selection and fallback.

**Related terms**: Capability, Capability Discovery, Module

---

## R

### Resource Manager

A Runtime Platform (Phase 3) subsystem that manages system resources: CPU, memory, I/O bandwidth, and storage. Handles resource discovery, allocation, limits, accounting, and contention resolution. Integrates with the Scheduler to ensure tasks have the resources they need.

**Related terms**: Runtime Platform, Scheduler, Task

### Runtime Platform

The operational layer of AI-native OS (Phase 3, completed). Provides the Scheduler, Supervisor, Session Manager, Task Manager, Context Manager, State Machine, Resource Manager, and Permission Checker. All higher-layer modules depend on the Runtime Platform.

**Related terms**: Core, Scheduler, Session, Supervisor, System Platform

---

## S

### Scheduler

A Runtime Platform (Phase 3) subsystem responsible for dispatching tasks to workers based on priority, resource availability, and scheduling policies. Uses a work-stealing multi-threaded model built on Tokio.

**Related terms**: Runtime Platform, Resource Manager, Task, Worker

### Security Context

The identity and permission metadata carried by every event and action in the system. Includes the principal (user, service, or agent), roles, capabilities, and authentication evidence. The Security Context is propagated through asynchronous boundaries by the Context Manager.

**Related terms**: Context Manager, Permission Checker, Policy

### Serialization

The process of converting event payloads and data structures to and from a portable format for transmission and storage. AI-native OS uses Serde (Serialize/Deserialize) as its serialization framework. All event payloads must implement Serialize and Deserialize.

**Related terms**: Event, Message

### Service

A runnable component within a module that performs a specific function. Services follow the standard lifecycle (Init, Starting, Running, Stopping, Stopped) managed by the Core Platform's Lifecycle subsystem. Services communicate by publishing and subscribing to events on the EventBus.

**Related terms**: Daemon, Lifecycle, Module, Supervisor

### Service Lifecycle

The standardized state machine that all services follow: Init, Starting, Running, Stopping, Stopped. The Lifecycle subsystem (Phase 2) manages state transitions and ensures services start and stop in dependency order. Failed services may be restarted by the Supervisor.

**Related terms**: Core, Daemon, Lifecycle, Service, Supervisor

### Session

A scoped context representing an interactive user session, application session, or system operation. Sessions are managed by the Session Manager (Phase 3) and carry identity, state, and resource allocations. Session lifecycle events are dispatched on the EventBus.

**Related terms**: Context, Context Manager, Session

### Span ID

A unique identifier for a unit of work within a distributed trace. Span IDs are generated by the Context Manager and propagated with events. A collection of spans sharing the same Trace ID forms a complete trace of an operation across module boundaries.

**Related terms**: Context, Context Manager, Trace ID

### State Machine

A Runtime Platform (Phase 3) subsystem that provides a generic event-driven state machine engine. Defines states, transitions, guards, and actions. Used to model workflows, session states, and business processes.

**Related terms**: Runtime Platform, Session, Task

### Supervisor

A Runtime Platform (Phase 3) subsystem that monitors service health and applies restart policies. Services register health check callbacks; the Supervisor polls them and takes action (restart, escalate, or shut down) based on the service's restart policy.

**Related terms**: Daemon Manager, Health Monitor, Service, Worker

### System Platform

The fourth layer of AI-native OS (Phase 4, in progress). Provides the Daemon Manager, Policy Engine, Capability Discovery, Configuration Manager, and Plugin System. Bridges the Runtime Platform services and the higher intelligence layers.

**Related terms**: Daemon, Policy, Plugin, Runtime Platform

---

## T

### Task

A discrete unit of work scheduled and executed by the platform. Tasks have an ID, type, priority, resource requirements, security context, and state. They are managed by the Task Manager (Phase 3) and executed by Workers. Tasks may be one-shot, recurring, or streaming.

**Related terms**: Resource Manager, Scheduler, Task ID, Worker

### Task ID

A globally unique identifier assigned to every task in the system. Used for tracking, correlation, and status queries. Task IDs are included in all events related to a task's lifecycle.

**Related terms**: Context, Span ID, Task, Trace ID

### Trace ID

A globally unique identifier for a distributed trace, spanning all events and operations related to a single root operation. Trace IDs are generated by the Context Manager and propagated through all descendant events. Every event in AI-native OS carries a Trace ID.

**Related terms**: Context, Context Manager, Span ID

---

## W

### Worker

A computational resource (thread, process, or container) that executes tasks. Workers are managed by the Task Manager and Scheduler. They register their availability and capabilities with the system and receive tasks dispatched by the Scheduler.

**Related terms**: Agent, Daemon, Scheduler, Supervisor, Task

---

## Appendix: Term Index by Category

### Architecture Concepts

- Architecture
- Clean Architecture
- Domain
- Layer
- Module
- Phase
- Plugin

### Communication

- Bus
- Event
- EventBus
- Handler
- Message
- Subscription

### Observability and Context

- Context
- Context Manager
- Logger
- Span ID
- Trace ID

### Security and Policy

- Capability
- Policy
- Policy Engine
- Permission Checker
- Security Context

### System Roles

- Agent
- Daemon
- Provider
- Service
- Worker

### Lifecycle Management

- Daemon Manager
- Health Monitor
- Lifecycle
- Service Lifecycle
- Supervisor

### Platform Layers (by Phase)

- Core (Phase 2)
- Runtime Platform (Phase 3)
- System Platform (Phase 4)
- Memory Platform (Phase 5)
- Brain Platform (Phase 6)
- Perception Platform (Phase 7)
- Execution Platform (Phase 8)
- Intelligence Integration (Phase 9)

### Resource and Execution

- Resource Manager
- Scheduler
- Session
- State Machine
- Task

---

## Appendix: Deprecated Terms

The following terms have been used in early project discussions but are deprecated in favor of the standard terms defined above:

| Deprecated Term | Preferred Term | Reason |
|---|---|---|
| Message Bus | EventBus | More precise: typed events, not generic messages |
| Health Checker | Health Monitor | Broader scope: monitoring, not just checking |
| Auth Service | Permission Checker | More precise: authorization, not authentication |
| Job | Task | Task is the standard term throughout the project |
| Module Manager | Daemon Manager | More precise: manages daemons, not generic modules |

---

*This glossary is maintained alongside the project documentation. New terms should be added as the project evolves. Deprecated terms should be moved to the appendix rather than removed, to aid in understanding historical documentation.*
