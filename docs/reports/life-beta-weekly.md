# LIFE Beta — Weekly Report

**Week of**: 2026-08-05
**Phase**: 4 — Real-World Companion Validation
**Status**: Policy gate live; E2E validation suite green

---

## 1. Summary

Phase 4 establishes LIFE as a validated daily companion. This week's work
delivers the **capability policy gate** — the mechanism that ensures LIFE
evaluates risk, requests confirmation for high-stakes actions, enforces
permissions, and audits every capability decision before dispatch.

The frozen runtime architecture (Runtime API, Runtime Manager, OpenClaw
Runtime, MCP) is untouched. The policy layer is new, additive, and lives
between cognition and the runtime manager.

## 2. Deliverables

| Artifact | Path | Status |
|---|---|---|
| Capability policy crate | `crates/capability-policy/` | ✅ 8 tests, clippy clean |
| Companion policy gate | `apps/desktop-companion/src/capability_policy.rs` | ✅ 4 tests, clippy clean |
| Startup wiring | `apps/desktop-companion/src/main.rs` | ✅ initialized at boot |
| Journal scaffolding | `docs/journal/life-beta/` | ✅ template + 2 entries |
| E2E validation suite | `tests/runtime-validation/` | ✅ real subprocess |

## 3. Cognitive Improvements

- **Confirmation discipline**: `email.send`, `github.create_issue`, and
  `calendar.create_event` are now `requires_confirmation = true`. LIFE
  holds the action instead of auto-executing, honoring the principle that
  externally-sent actions require user approval.
- **Sandbox containment**: `filesystem.write_root` is permanently `denied`,
  so no plan can smuggle a write outside the filesystem plugin root through
  the runtime.
- **Permission check before dispatch**: the gate verifies caller authorities
  (`network.outbound`, `email.compose`, `filesystem.write`, etc.) before any
  capability is allowed; missing permissions surface as typed denials, not
  runtime failures.

No changes were made to the Cognitive Loop, Planner, World Understanding,
Memory Evaluator, or Evolution Engine — cognition remains vendor- and
policy-agnostic.

## 4. Capability Usage (policy surface)

The companion initialized with the default plugin capability set. The policy
registry classifies each capability:

| Capability | Risk | Requires confirmation | Notes |
|---|---|---|---|
| `calendar.read` | low | no | read-only |
| `github.read_repository` | low | no | read-only |
| `filesystem.read` | medium | no | read under sandbox |
| `telegram.send` | medium | no | outbound chat |
| `email.send` | **high** | **yes** | external |
| `calendar.create_event` | **high** | **yes** | external |
| `github.create_issue` | **high** | **yes** | external |
| `filesystem.write_root` | critical | — | **denied** |
| `telegram.inject_inbound` | low | no | sim only |
| `email.inject` | low | no | sim only |

The audit log records every `evaluate` call. At startup, LIFE evaluates the
four headline capabilities and logs each decision (risk, confirmation flag,
allowed) so the journal has an auditable trail.

## 5. User Friction

- **Confirmation prompt surface**: the companion currently evaluates the
  policy at startup but the per-action confirmation UI (web or Telegram) is
  not yet wired into the dispatch path. This is the next integration point —
  the dispatch path must call `gate.evaluate(capability)` and await explicit
  user approval for any `requires_confirmation` capability. **Open**.
- **Policy is static**: the registry is compiled from the default plugin set.
  Adding a new plugin requires adding its policy entry. Long term, policies
  should load from a manifest, but for beta a static table is auditable and
  reviewable.

## 6. Trust Issues

No trust incidents this week. The policy gate prevents the class of incident
where a mis-planned capability (e.g. `filesystem.write_root`) would silently
execute — such capabilities are now denied before reaching the runtime.
The separation between cognition and policy means a planner error cannot
bypass a security boundary.

## 7. Proposed Behavioral Improvements

1. **Wire the gate into dispatch**: `RuntimeManager::dispatch` calls should be
   preceded by `CapabilityGate::evaluate`, with confirmation requested via
   the web UI / Telegram for `requires_confirmation` capabilities.
2. **Dynamic policy load**: allow `config/capabilities.toml` to override the
   compiled-in policy table per-deployment.
3. **Daily journaling cadence**: run LIFE as the primary companion for daily
   scenarios (email, calendar, reminders, GitHub issues, document search) and
   append one structured journal entry per session to
   `docs/journal/life-beta/`.

## 8. Validation Evidence

```
$ cargo test -p ai-os-capability-policy
   ... 8 passed

$ cargo test -p desktop-companion capability_policy
   ... 4 passed

$ cargo test -p runtime-validation-tests
   (bootstrap, recovery, cognitive_loop scenarios against real MCP subprocess)
```

The runtime validation suite exercises the real `ai-os-mcp-server` subprocess
end to end: capability discovery, dispatch, observation ingestion, and the
full inbound-telegram → Cognitive Loop → World Model flow. Together with the
policy gate, this proves LIFE can both reason (cognition) and act safely
(policy).

## 9. Success Definition

> The measure of success is not the number of capabilities. Success is when
> LIFE becomes a companion you naturally rely on every day because it
> remembers, understands, exercises good judgment, and uses its capabilities
> appropriately.

This week delivers the "good judgment / uses capabilities appropriately"
half: LIFE now has an explicit, auditable policy over its capabilities and
demonstrates correct confirmation discipline over a real subprocess. The
weekly report continues next week with real daily-use journals.
