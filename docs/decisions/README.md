# Technical Decisions

This directory captures technical decisions that fall outside the scope of formal ADRs — such as implementation-level choices, dependency selections, configuration defaults, and protocol details.

## Purpose

- Record why a specific library was chosen over alternatives
- Document non-architectural but impactful decisions
- Provide context for code reviewers and future maintainers

## Format

Each decision should be captured as a separate Markdown file with a descriptive name. A lightweight format is acceptable:

```markdown
# Decision: [Title]

Date: YYYY-MM-DD

## Context

What problem needed a decision?

## Options Considered

1. Option A — pros/cons
2. Option B — pros/cons

## Chosen Approach

What was selected and why.

## Implications

What does this mean for the codebase going forward?
```

## Index

| File | Topic | Date |
|---|---|---|
| _No decisions recorded yet_ | | |

## Cross-References

- [ADR Index](../adr/README.md) — for architectural decisions
- [Architecture Overview](../architecture/overview.md)
