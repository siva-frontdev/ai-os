# Architecture Decision Records

This directory contains the Architecture Decision Record (ADR) collection for the AI-native Operating Platform.

## What is an ADR?

An Architecture Decision Record is a short document capturing an important architectural decision made during the project lifecycle, along with its context, consequences, and compliance criteria. ADRs serve as the permanent record of why the system is built the way it is.

Each ADR describes a specific architectural choice, the forces that motivated it, the alternatives considered, and the trade-offs accepted. ADRs are not design documents — they capture decisions, not designs.

## ADR Lifecycle

Every ADR passes through a lifecycle of states:

```
[Proposed] --> [Accepted] --> [Deprecated]
                  |
                  +--> [Superseded] (by a newer ADR)
```

| State        | Meaning                                                  |
|--------------|----------------------------------------------------------|
| Proposed     | Under discussion, not yet adopted.                       |
| Accepted     | Formally adopted and the implementation follows it.      |
| Deprecated   | Still in effect but should no longer be followed for new work. |
| Superseded   | Replaced by a newer ADR. The superseding ADR references this one. |

A decision moves from Proposed to Accepted through team consensus (lazy consensus model: if no objections within one week, the decision is accepted).

## Numbering Scheme

ADRs are numbered sequentially (0001, 0002, ...). The numbering is immutable — once assigned, an ADR retains its number even if superseded. This guarantees stable cross-references.

When an ADR is superseded, the new ADR receives the next available sequential number. The superseded ADR's status is updated, and it links forward to the superseding ADR.

## Creating a New ADR

To propose a new architectural decision:

1. Copy the template file (`template.md`) to `NNNN-title-of-decision.md`, where `NNNN` is the next available number.
2. Fill in all sections. Every section must be present; use "N/A" if a section has no content.
3. Set the Status field to "Proposed".
4. Submit as a pull request or patch for review.

Required sections:

- **Title**: A clear, concise name for the decision.
- **Status**: Current lifecycle state.
- **Date**: ISO 8601 date (YYYY-MM-DD) of the decision.
- **Context**: The problem being solved, forces at play, and constraints.
- **Decision**: The choice that was made. Be explicit.
- **Consequences**: The positive and negative outcomes of the decision.
- **Compliance**: How to verify the decision is being followed in the codebase.
- **Notes**: Follow-up items, amendments, or discussion references.
- **References**: Links to related ADRs, external documents, or source files.

## Existing ADRs

| ID     | Title                                    | Status     | Date       |
|--------|------------------------------------------|------------|------------|
| 0001   | Project Vision and Scope                 | Accepted   | 2025-01-15 |
| 0002   | Clean Architecture with Layered Modules  | Accepted   | 2025-01-20 |
| 0003   | Rust as Implementation Language          | Accepted   | 2025-01-22 |
| 0004   | Event-Driven Architecture via EventBus   | Accepted   | 2025-01-25 |
| 0005   | Runtime Platform Design                  | Accepted   | 2025-02-01 |

## References

- [ADR Template](./template.md)
- [MADR: Markdown Any Decision Records](https://adr.github.io/madr/)
- [Documentation Index](../README.md)
