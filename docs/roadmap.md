# Development Roadmap

## Overview

AI-native OS is developed in nine phases across a multi-year timeline. Each phase builds on the deliverables of previous phases, following the layered architecture defined in the [architecture document](architecture.md). Phases 1 through 3 are complete. Phase 4 is in active development. Phases 5 through 9 are planned.

The phases correspond to the architectural layers:

| Phase | Layer | Status |
|---|---|---|
| Phase 1 | Dev Environment | Completed |
| Phase 2 | Core Platform | Completed |
| Phase 3 | Runtime Platform | Completed |
| Phase 4 | OSAL (Operating System Abstraction Layer) | In Progress |
| Phase 5 | Memory Platform | Planned |
| Phase 6 | Brain Platform | Planned |
| Phase 7 | Perception Platform | Planned |
| Phase 8 | Execution Platform | Planned |
| Phase 9 | Intelligence Integration | Planned |

---

## Phase 1: Dev Environment

**Status**: Completed

### Objectives

- Establish the Rust development environment and toolchain.
- Configure the build system with Cargo workspace.
- Set up continuous integration and continuous delivery (CI/CD) pipelines.
- Create a reproducible development container or environment definition.
- Establish coding standards, linting, and formatting configurations.

### Key Deliverables

| Deliverable | Description |
|---|---|
| Cargo workspace | Multi-crate workspace with shared dependency management |
| CI/CD pipeline | GitHub Actions or equivalent: build, test, lint, format, security audit |
| Dev container | Dockerfile and devcontainer.json for reproducible development |
| Rust toolchain config | rust-toolchain.toml specifying stable Rust version |
| Linting config | Clippy configuration with project-specific lint levels |
| Format config | rustfmt configuration matching project style |
| PR template | Pull request template with checklist |
| Contributing guide | CONTRIBUTING.md with development workflow |

### Dependencies

None. This phase bootstraps the project.

### Estimated Effort

Low. Standard project setup with established patterns.

---

## Phase 2: Core Platform

**Status**: Completed

### Objectives

- Implement the EventBus as the central communication backbone.
- Build the Service Lifecycle system for managing component state.
- Create the structured Logger with tracing integration.
- Implement the HealthMonitor for liveness and readiness checks.
- Build the Container abstraction for managing containerized workloads.

### Key Deliverables

| Deliverable | Description |
|---|---|
| EventBus | Typed event dispatch, subscription management, middleware pipeline |
| Service Lifecycle | Five-state lifecycle (Init, Starting, Running, Stopping, Stopped) |
| Logger | Structured logging with levels, spans, fields, and multiple sinks |
| HealthMonitor | Health check registration, probe endpoints, status aggregation |
| Container | Container runtime abstraction (Docker/Podman API) |

### Dependencies

Phase 1 (Dev Environment).

### Estimated Effort

Medium. Core infrastructure requires careful API design and thorough testing.

### Notes

The EventBus is the most critical deliverable of this phase. All subsequent phases depend on its correctness and performance. Extensive property-based testing was applied to event routing, subscription matching, and middleware execution.

---

## Phase 3: Runtime Platform

**Status**: Completed

### Objectives

- Implement the Scheduler for task dispatch and coordination.
- Build the Supervisor for service health monitoring and restart policies.
- Create the Session Manager for user and application session lifecycle.
- Implement the Task Manager for task queuing, dispatch, and tracking.
- Build the Context Manager for distributed tracing and span propagation.
- Implement the State Machine engine for workflow orchestration.
- Build the Resource Manager for CPU, memory, and I/O allocation.
- Implement the Permission Checker for access control evaluation.

### Key Deliverables

| Deliverable | Description |
|---|---|
| Scheduler | Priority queue, work stealing, deadline scheduling |
| Supervisor | Health polling, restart policies (always, on-failure, never) |
| Session Manager | Session creation, authentication, timeout, teardown |
| Task Manager | Task queues, workers, progress tracking, completion events |
| Context Manager | Trace ID and Span ID generation and propagation |
| State Machine | Event-driven state machine definition and execution engine |
| Resource Manager | Resource discovery, allocation, limits, accounting |
| Permission Checker | Policy evaluation engine, allow/deny decisions, audit logging |

### Dependencies

Phase 2 (Core Platform).

### Estimated Effort

High. This is the largest completed phase, encompassing eight distinct subsystems.

### Notes

The Runtime Platform represents the operational backbone of AI-native OS. Every component above this layer depends on its services. The Scheduler and Permission Checker received the most extensive testing due to their criticality.

---

## Phase 4: OSAL — Operating System Abstraction Layer

**Status**: In Progress

### Objectives

- Implement the Daemon Manager for long-running background services.
- Build the Policy Engine for rule definition, evaluation, and enforcement.
- Create the Capability Discovery system for module capability registration.
- Implement the Configuration Manager for layered, runtime-updatable config.
- Build the Plugin System for dynamic module loading and isolation.

### Key Deliverables

| Deliverable | Description | Progress |
|---|---|---|
| Daemon Manager | Start, stop, monitor daemon processes; dependency ordering | Core implemented |
| Policy Engine | Rule definition DSL, evaluation engine, enforcement hooks | In design |
| Capability Discovery | Registration, query, versioning of module capabilities | In development |
| Configuration Manager | Multi-source configuration, runtime reload, validation | Core implemented |
| Plugin System | Dynamic loading, sandboxing, lifecycle management | In design |

### Dependencies

Phase 3 (Runtime Platform).

### Estimated Effort

High. The Policy Engine and Plugin System are particularly complex, requiring careful security design.

### Key Design Decisions

- The Policy Engine uses a declarative rule language defined in TOML/YAML, not a custom DSL, to reduce learning curve.
- Plugins are loaded as separate processes (not dynamic libraries) to enforce memory safety isolation. Communication with plugins occurs through the EventBus via IPC transport.
- Configuration changes are propagated as events on the EventBus, enabling real-time updates without service restart.

---

## Phase 5: Memory Platform

**Status**: Planned

### Objectives

- Design and implement persistent memory stores for learned patterns.
- Build a query engine for efficient retrieval of stored memory.
- Implement memory lifecycle management (creation, consolidation, pruning).
- Create the memory event stream for capturing and replaying system history.
- Implement memory embeddings for similarity-based retrieval.

### Key Deliverables

| Deliverable | Description |
|---|---|
| Persistent Store | Durable, queryable storage for learned patterns and state |
| Query Engine | Structured and similarity-based querying |
| Memory Lifecycle | Memory creation, consolidation, decay, and pruning |
| Event Stream Store | Append-only event log for history and replay |
| Embedding Service | Vector embedding generation and similarity search |

### Dependencies

Phase 4 (System Platform).

### Estimated Effort

High. Storage infrastructure is inherently complex, and the embedding/query requirements add significant design surface.

### Anticipated Challenges

- Balancing durability with performance for real-time memory operations.
- Designing memory decay and consolidation policies that preserve important patterns while managing storage growth.
- Choosing the storage backend (embedded vs. external DB) based on Phase 4 integration results.

---

## Phase 6: Brain Platform

**Status**: Planned

### Objectives

- Implement the core reasoning engine for decision-making.
- Build the state modeling system for maintaining system state representations.
- Create the learning feedback loop for continuous improvement.
- Implement the inference engine for real-time event interpretation.
- Build the meta-reasoning layer for self-assessment and confidence estimation.

### Key Deliverables

| Deliverable | Description |
|---|---|
| Reasoning Engine | Rule-based and probabilistic reasoning |
| State Modeler | Graph-based system state representation and updates |
| Learning Loop | Feedback collection, model update, evaluation |
| Inference Engine | Real-time event processing and interpretation |
| Meta-Reasoning | Confidence estimation, uncertainty handling |

### Dependencies

Phase 5 (Memory Platform). The Brain Platform requires persistent memory for learning and state modeling.

### Estimated Effort

High. This phase represents the first direct integration of AI/ML concepts into the platform.

### Anticipated Challenges

- Ensuring inference latency meets real-time system requirements.
- Designing the learning loop to avoid catastrophic forgetting.
- Integrating with the Permission Checker to ensure the Brain does not bypass security controls.
- Making the reasoning engine explainable for debugging and audit.

---

## Phase 7: Perception Platform

**Status**: Planned

### Objectives

- Implement sensor integration framework for diverse data sources.
- Build signal processing pipelines for data transformation.
- Create pattern recognition services for anomaly and event detection.
- Implement sensor fusion for combining multiple data sources.
- Build the perception event stream for processed sensor data.

### Key Deliverables

| Deliverable | Description |
|---|---|
| Sensor Framework | Sensor registration, data ingestion, normalization |
| Signal Processing | Filtering, transformation, feature extraction pipelines |
| Pattern Recognition | Anomaly detection, event classification, trend analysis |
| Sensor Fusion | Multi-sensor data combination and correlation |
| Perception Stream | Processed, annotated perception events |

### Dependencies

Phase 6 (Brain Platform). Perception provides input to the reasoning engine and requires Brain interface contracts.

### Estimated Effort

Medium to High. Signal processing is well-understood; integration with the event-driven architecture is the primary challenge.

### Anticipated Challenges

- Handling diverse sensor data formats and rates.
- Real-time processing of high-volume sensor streams.
- Sensor failure detection and graceful degradation.

---

## Phase 8: Execution Platform

**Status**: Planned

### Objectives

- Implement autonomous action planning and execution.
- Build multi-agent coordination for distributed task execution.
- Create the action library for system-level operations.
- Implement execution monitoring and rollback.
- Build the human-in-the-loop interface for supervised execution.

### Key Deliverables

| Deliverable | Description |
|---|---|
| Action Planner | Goal decomposition, plan generation, plan optimization |
| Agent Coordinator | Multi-agent task assignment, communication, conflict resolution |
| Action Library | Catalog of system actions with preconditions and effects |
| Execution Monitor | Plan execution tracking, deviation detection, rollback |
| Human Interface | Approval workflows, override controls, explanation generation |

### Dependencies

Phase 7 (Perception Platform). Execution requires perception to inform decisions and evaluate outcomes.

### Estimated Effort

High. Autonomous execution in a system context requires rigorous safety guarantees.

### Anticipated Challenges

- Ensuring execution safety: plans must not violate system integrity.
- Handling partial failures during plan execution.
- Designing the human-in-the-loop interface for appropriate oversight without bottlenecking autonomy.
- Rollback semantics: not all actions are reversible.

---

## Phase 9: Intelligence Integration

**Status**: Planned

### Objectives

- Unify Memory, Brain, Perception, and Execution into a cohesive intelligence layer.
- Implement self-optimization across all platform layers.
- Build self-healing capabilities for autonomous fault recovery.
- Create the self-configuration system for adaptive tuning.
- Implement the learning infrastructure for continuous platform improvement.

### Key Deliverables

| Deliverable | Description |
|---|---|
| Unified Intelligence Layer | Integrated API across all intelligence subsystems |
| Self-Optimization Engine | Performance analysis, tuning recommendation, automated adjustment |
| Self-Healing System | Fault detection, diagnosis, remediation planning, execution |
| Self-Configuration | Automatic parameter tuning, resource allocation, topology optimization |
| Learning Infrastructure | Cross-subsystem learning, knowledge transfer, curriculum management |

### Dependencies

Phase 8 (Execution Platform). Integration requires all subsystems to be operational.

### Estimated Effort

Very High. Integration complexity is proportional to the number of interacting subsystems.

### Anticipated Challenges

- Integration testing across nine phases of interdependent subsystems.
- Ensuring consistent behavior when multiple subsystems make conflicting decisions.
- Measuring and demonstrating the value of integrated intelligence compared to isolated subsystems.
- Managing the meta-stability of a system that modifies its own behavior.

---

## Visual Timeline

The following timeline shows the approximated development schedule across phases. Milestones are targets and may shift based on complexity discovered during development.

```
Year 1                Year 2                Year 3                Year 4                Year 5+
|                     |                     |                     |                     |
[Phase 1: Dev Env]    |                     |                     |                     |
  |--- COMPLETED ---> |                     |                     |                     |
                      |                     |                     |                     |
[Phase 2: Core]       |                     |                     |                     |
  |--- COMPLETED ---> |                     |                     |                     |
                      |                     |                     |                     |
[Phase 3: Runtime]    |                     |                     |                     |
  |--- COMPLETED ---> |                     |                     |                     |
                      |                     |                     |                     |
[Phase 4: System]     |                     |                     |                     |
   ===== IN PROGRESS =========>             |                     |                     |
                      |                     |                     |                     |
                      [Phase 5: Memory]     |                     |                     |
                      |   ===== PLANNED ===========>             |                     |
                      |                     |                     |                     |
                      [Phase 6: Brain]      |                     |                     |
                      |   ===== PLANNED ===========>             |                     |
                      |                     |                     |                     |
                      [Phase 7: Perception] |                     |                     |
                      |   ===== PLANNED ===========>             |                     |
                      |                     |                     |                     |
                      [Phase 8: Execution]  |                     |                     |
                      |   ===== PLANNED ================================>              |
                      |                     |                     |                     |
                      [Phase 9: Integration]|                     |                     |
                      |   ===== PLANNED ==============================================>|
```

### Phase Duration Estimates

| Phase | Estimated Duration | Notes |
|---|---|---|
| Phase 1 | 2-3 months | Standard project setup |
| Phase 2 | 4-6 months | Core infrastructure, extensive testing |
| Phase 3 | 6-9 months | Eight subsystems, critical integration |
| Phase 4 | 6-9 months | Policy engine and plugin complexity |
| Phase 5 | 6-9 months | Storage infrastructure, embedding design |
| Phase 6 | 9-12 months | First AI integration, research required |
| Phase 7 | 6-9 months | Sensor integration, signal processing |
| Phase 8 | 9-12 months | Autonomous execution, safety guarantees |
| Phase 9 | 12-18 months | Integration, self-*, production hardening |

### Key Milestones

| Milestone | Target | Phase |
|---|---|---|
| First event dispatched on EventBus | Achieved | Phase 2 |
| First service lifecycle completed | Achieved | Phase 2 |
| Runtime platform operational | Achieved | Phase 3 |
| Policy engine operational | 2026 Q4 | Phase 4 |
| Plugin system operational | 2027 Q1 | Phase 4 |
| Persistent memory store | 2027 Q3 | Phase 5 |
| First reasoning engine inference | 2028 Q1 | Phase 6 |
| Real-time perception pipeline | 2028 Q3 | Phase 7 |
| Autonomous action executed | 2029 Q1 | Phase 8 |
| Full intelligence integration | 2030+ | Phase 9 |

---

## Dependencies Between Phases

```
Phase 1 (Dev Env)
  |
  v
Phase 2 (Core)
  |
  v
Phase 3 (Runtime)
  |
  v
Phase 4 (System)
  |
  +--------+--------+
  |        |        |
  v        v        v
Phase 5  Phase 6  Phase 7
  |        |        |
  +--------+--------+
           |
           v
      Phase 8 (Execution)
           |
           v
      Phase 9 (Integration)
```

Phases 5, 6, and 7 have a partial parallelization opportunity. While they depend on Phase 4, they do not strictly depend on each other. However, the interfaces between Memory, Brain, and Perception must be designed collaboratively to ensure integration in Phases 8 and 9 does not require rework.

---

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Phase 4 Policy Engine complexity exceeds estimates | Medium | High | Prototype early; consider simpler initial policy model |
| Phase 5 storage backend choice blocks progress | Medium | Medium | Design storage abstraction layer early; evaluate options in Phase 4 |
| Phase 6 AI integration research uncertainty | High | High | Start ML research in Phase 4; partner with academic researchers |
| Phase 8 safety guarantees limit autonomy | Medium | High | Implement graduated autonomy: supervised -> assisted -> autonomous |
| Integration complexity in Phase 9 | High | Very High | Continuous integration from Phase 5 onward; never integrate all at once |
| Contributor burnout on 10-year project | Medium | High | Maintain clear milestones, celebrate completions, rotate responsibilities |

---

## Summary

The AI-native OS roadmap spans nine phases from development environment through full intelligence integration. Three phases are complete. One is in progress. Five remain. Each phase is designed to produce a working, testable system that builds on the layers beneath it.

This roadmap is a living document. As the project progresses, estimates will be refined, deliverables will be adjusted, and the timeline will be updated. The architecture and principles provide the stable foundation that allows the roadmap to adapt without compromising the vision.

Refer to the [architecture document](architecture.md) for detailed descriptions of each phase's modules and the [principles document](principles.md) for the design framework that guides implementation.
