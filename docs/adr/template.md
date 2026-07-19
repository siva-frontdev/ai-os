# ADR-NNNN: [Short title of the decision]

## Status

[Proposed | Accepted | Deprecated | Superseded]

If Superseded, include: Superseded by [ADR-NNNN](./NNNN-title.md).

## Date

[YYYY-MM-DD when the decision was last updated]

## Context

[Describe the problem that motivated this decision. What forces are at play — technical, organizational, scheduling? What constraints must the decision respect? Include relevant background information so a future reader understands why the decision was made.

Typical elements:
- The specific problem or opportunity being addressed.
- Any alternatives that were considered and why they were ultimately rejected.
- Technical constraints (language, platform, performance targets, etc.).
- Business or organizational context that influenced the decision.
- Dependencies on other ADRs or prior decisions.

Keep this section factual and neutral. The goal is to capture the full picture so that the decision is understandable without additional context.

Be thorough. A well-written Context section is the most valuable part of an ADR.]

## Decision

[State the decision explicitly. Use concrete language: "We will ..." rather than "We considered ...". This section should be a clear, unambiguous statement of the chosen approach.

Include:
- The specific technology, pattern, or architectural approach selected.
- Key configuration or design parameters (e.g., which version, which defaults).
- How this decision interacts with existing architectural decisions.
- Any scope limitations or exclusions.

Example: "We will implement inter-module communication using a centralized EventBus with TypeId-based message routing and async dispatch. Each module publishes events to the bus and subscribes to event types it handles. Direct function calls between modules are prohibited."]

## Consequences

[Describe the resulting context after applying the decision. Use two subsections:

### Positive

- [Benefit 1: e.g., "Loose coupling between modules enables independent development and testing."]
- [Benefit 2]
- [Benefit 3]

### Negative

- [Trade-off 1: e.g., "Event ordering is not guaranteed; modules must handle out-of-order delivery."]
- [Trade-off 2]
- [Trade-off 3]

Be honest about trade-offs. Every architectural decision involves compromises. Documenting them helps future teams understand why certain costs were accepted.]

## Compliance

[Describe how to verify that the decision is being followed in the codebase. This section should contain actionable checks that can be automated or performed during code review.

Examples:
- "All inter-module communication must go through the EventBus. Code review should reject direct imports between module crates (except the core crate)."
- "A CI lint rule enforces that no trait method exceeds 50 lines."
- "The dependency graph must remain acyclic. Run `cargo metadata` and verify with a graph analysis tool in CI."

If automated enforcement exists, reference the configuration or script.]

## Notes

[Any follow-up items, amendments, or links to discussion threads. This section captures the ongoing history of the decision.

Examples:
- "Amended 2025-03-01: Added compliance section per team review."
- "Discussion thread: https://example.com/issues/42"
- "Follow-up: ADR-NNNN will address the event ordering guarantee."
- "This decision was revisited during Phase 4 planning; confirmed still valid."

If there are no notes, write "N/A".]

## References

- [Related ADR or document title](./NNNN-title.md)
- [External link or internal documentation path](../path/to/doc.md)
- [Source file or module path](../../src/path/to/module.rs)

[List all references in a bullet list. Include at minimum any ADRs that this decision builds upon, conflicts with, or supersedes.]
