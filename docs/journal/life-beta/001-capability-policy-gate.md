# Behavior Journal: LIFE Beta

---

## Entry 001 — Capability policy gate comes online

**Date**: 2026-08-05
**Session start**: 13:55 UTC
**Capabilities available**: email.send, email.inject, filesystem.read,
filesystem.write, filesystem.search, github.create_issue,
github.read_repository, calendar.read, calendar.create_event,
telegram.send, telegram.inject_inbound
**Policy gate**: initialized

### Situation

The user's mission: LIFE must become a daily companion that remembers,
understands, exercises good judgment, and uses its capabilities appropriately.
Before any real-world use, the companion must never act dangerously — it must
evaluate risk, request confirmation for high-stakes actions, enforce
permissions, and audit every capability decision.

### Observation

The runtime dispatch path (Phase 3) produces a flat capability surface. The
Cognitive Loop plans against abstract capability ids, but there was no layer
that answered: "is this safe to run, and does the caller have the right to?"
A `filesystem.write_root` capability would have walked straight through
without a gate.

### Understanding

This is a policy gap, not a cognition bug. Cognition correctly plans against
capability ids; the missing piece is the policy gate that sits between the
plan and `RuntimeManager::dispatch`. Per Phase 4's architecture, the gate is
the only place policy is evaluated, keeping the cognitive core clean.

The relevant code is the new `CapabilityGate` at
`apps/desktop-companion/src/capability_policy.rs`. It builds a `PolicyRegistry`
(one policy per real plugin capability) and a `CallerPermissions` set seeded
from the companion's default authorities, and records every decision into an
`AuditLog` (from `crates/capability-policy`).

Policy rules applied:
- Read-only/low-risk (`calendar.read`, `telegram.inject_inbound`) → execute
  without confirmation, low risk.
- State-mutating (`email.send`, `github.create_issue`, `calendar.create_event`)
  → High risk, **require confirmation**.
- Sandbox-escape (`filesystem.write_root`) → `denied` (overrides everything).

### Decision

Introduce `crates/capability-policy` (a new crate, no changes to the frozen
Runtime API / Runtime Manager / OpenClaw / MCP). Re-export
`CapabilityId`/`RuntimeId` from the policy crate so the companion depends on
the policy layer, not on the runtime API directly. Wire the gate into
`desktop-companion/src/main.rs` at startup: it constructs the gate, evaluates
the default plugin capabilities so the audit log starts populated, and logs
each decision. The real per-action evaluate call plugs into the runtime
dispatch path when Phase 3 wiring lands in the companion.

Verification: ran the crate's unit tests (8 in the policy crate, 4 in the
companion module) — all pass. Clippy clean (`-D warnings`) for the new code.

### Outcome

Resolved. LIFE now has an explicit, auditable policy gate over its capabilities.
The companion starts with the policy surface populated and the audit log
recording every capability decision. The next step is real-world use: run
LIFE through actual Telegram scenarios and record cognitive behavior.
