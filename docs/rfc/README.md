# Request for Comments (RFC)

RFCs are the mechanism for proposing, discussing, and reaching consensus on significant changes to the AI-native OS platform. They precede implementation and ensure architectural decisions are reviewed before code is written.

## The Flow

```
Idea (problem or opportunity identified)
  │
  ▼
RFC (written proposal)
  │
  ▼
Review (community discussion, feedback, iteration)
  │
  ▼
Decision (Accepted / Rejected / Postponed)
  │
  ▼
Implementation (code written against the accepted RFC)
  │
  ▼
ADR (architectural decision recorded after implementation)
  │
  ▼
Specification (only if the change affects fundamental contracts)
```

An RFC proposes what to build and why. An ADR records what was actually decided during implementation. The Specification changes only when fundamental contracts are affected.

## When to Write an RFC

Write an RFC when:

- Adding a new module or crate
- Changing a cross-module interface or trait contract
- Introducing a new external dependency with significant scope
- Changing the event format, EventBus semantics, or inter-module communication pattern
- Changing the boot sequence or shutdown sequence
- Adding a new security mechanism or changing the authorization model
- Adding a new performance target or changing existing targets
- Any change that affects multiple modules or phases

Do NOT write an RFC for:

- Bug fixes
- Internal refactoring that does not change public interfaces
- Adding tests or documentation
- Dependency updates (unless for a new major-version dependency)
- Configuration changes within an existing module

## RFC Lifecycle

| Stage | Description |
|---|---|
| **Draft** | RFC is being written. Not yet ready for review. |
| **Review** | RFC is submitted for discussion. Feedback period is 14 days minimum. |
| **Accepted** | Consensus reached. Implementation may begin. |
| **Rejected** | Proposal declined with rationale. May be resubmitted after addressing concerns. |
| **Postponed** | Deferred to a later phase. Not rejected, but not actionable now. |
| **Superseded** | Replaced by a newer RFC. |

## Numbering

RFCs are numbered sequentially: `RFC-NNNN-descriptive-name.md`. The number is assigned when the RFC enters **Review** stage.

## Current RFCs

| RFC | Title | Status | Phase |
|---|---|---|---|
| RFC-0001 | OSAL — Operating System Abstraction Layer | Accepted | Phase 4 |
| RFC-0002 | Memory Platform | Draft | Phase 5 |
| RFC-0003 | Brain Platform | Draft | Phase 6 |

## Cross-References

- [ADR Index](../adr/README.md) — decisions recorded after implementation
- [Specification](../specification.md) — constitutional contracts
- [Architecture Overview](../architecture/overview.md)
