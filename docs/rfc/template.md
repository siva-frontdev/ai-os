# RFC-NNNN: Title

| Field | Value |
|---|---|
| **Status** | Draft / Review / Accepted / Rejected / Postponed / Superseded |
| **Author** | Name or GitHub handle |
| **Phase** | Phase number this RFC belongs to |
| **Created** | YYYY-MM-DD |
| **Updated** | YYYY-MM-DD |
| **Requires** | RFC numbers that must be accepted first (or None) |
| **Supersedes** | RFC numbers this proposal replaces (or None) |

## Abstract

A brief summary (2-3 sentences) of what this RFC proposes and why.

## Motivation

Why is this change needed? What problem does it solve? What happens if we do nothing?

## Design

### Overview

High-level description of the proposed design. Diagrams (ASCII or PlantUML) are encouraged.

### Detailed Design

#### Interfaces

New traits, types, and public API surfaces. Use Rust-like pseudocode:

```rust
pub trait NewCapability: Debug + Send + Sync {
    fn operation(&self, param: &str) -> Result<Output, Error>;
}
```

#### Events

Events this module publishes and consumes. Include event type strings and payload structures.

| Direction | Event Type | Payload | Description |
|---|---|---|---|
| Published | `module.event_name` | `EventPayload { ... }` | When and why this event is emitted |
| Consumed | `other.event_name` | `OtherPayload { ... }` | What this module does with the event |

#### Dependencies

Internal and external dependencies. For each: what it provides and why it is needed.

| Dependency | Layer | Purpose |
|---|---|---|
| `ai_os_core` | Core | EventBus, Service trait, Logger, HealthMonitor |
| `ai_os_runtime` | Runtime | Task scheduling, supervision, permissions |

#### Configuration

Configuration keys, types, defaults, and validation rules.

| Key | Type | Default | Description |
|---|---|---|---|
| `module.setting` | `u32` | `100` | Brief description of the setting |

#### Thread Model

How this module uses Tokio tasks, which primitives are used for shared state, and how contention is managed.

#### Lifecycle

How this module integrates with the Service lifecycle: init, start, stop. What happens at each phase.

#### Error Handling

Error types and recovery strategies. Which failures are transient, which are fatal.

### Security Considerations

How this design addresses authentication, authorization, audit, and data protection. Any new attack surface introduced.

### Performance Considerations

Expected performance characteristics. New performance targets, if any. Resource consumption estimates.

### Testing Strategy

How the module will be tested: unit tests, integration tests, benchmarks, property-based tests.

## Drawbacks

Why might we NOT do this? What are the trade-offs?

## Alternatives Considered

What other approaches were evaluated and why were they rejected?

| Alternative | Reason for Rejection |
|---|---|
| Approach A | Does not satisfy requirement X |
| Approach B | Introduces unacceptable complexity |

## Open Questions

Questions that need to be resolved before implementation begins.

1. Question about design decision X?
2. Question about dependency Y?

## Implementation Plan

Rough order of implementation steps, estimated effort, and dependencies.

1. Step one (estimated effort: small/medium/large)
2. Step two (estimated effort: small/medium/large)
3. Step three (estimated effort: small/medium/large)

## Unresolved Topics

Topics explicitly out of scope for this RFC that will be addressed separately.

## References

- Links to related RFCs, ADRs, issues, or external documents
- [Specification](../specification.md) section references
- [Architecture](../architecture/overview.md) document references
