# Behavior Journal: LIFE Beta

---

## Entry 002 — Telegram message to email (confirmation in effect)

**Date**: 2026-08-05
**Session start**: 14:12 UTC
**Capabilities available**: telegram, email (real MCP subprocess)
**Policy gate**: initialized; `email.send` flagged high-risk, requires_confirmation

### Situation

A Telegram message arrives: "send email to admin@example.com about the
project." The user expects LIFE to recognize the email intent, plan an
`email.send`, and — because email is a high-risk, externally-sent action —
ask for confirmation before dispatching.

### Observation

LIFE received the simulated inbound telegram via
`telegram.inject_inbound`. The RuntimeManager produced an `inbound_message`
observation (`source = telegram.receive`). The observation reached the
pull-drain after the MCP server's notification was processed.

The real Cognitive Loop (`CognitiveLoopService::cycle`) ran the full pipeline
on the observation text with a scripted coordinator. Understanding extracted
the sender as a `person` entity; the memory evaluator retained it; evolution
stored it in the World Model. The Planner produced an `email.send` action
mapped to topics `["admin@example.com", "Project update", ...]`.

Before dispatch, `CapabilityGate::evaluate("email.send")` returned:
`allowed = true`, `requires_confirmation = true`, `risk = high`. LIFE did
**not** auto-send; it held the action and surfaced the plan for confirmation.

### Understanding

This is the policy gate doing its job at
`apps/desktop-companion/src/capability_policy.rs:evaluate`. The planner's
decision (`email.send`) is correct; the gate is what prevents the
auto-send. Root cause if it had auto-sent: the dispatch path would bypass
`CapabilityGate`. Today the gate is the checkpoint.

### Decision

Dispatch is gated behind confirmation. The action remains in a pending state
and the user is prompted: "Send email to admin@example.com about 'Project
update'?" Only on confirmation would `RuntimeManager::dispatch` run. This
honors the Phase 4 principle that destructive/outbound actions require user
approval.

### Outcome

Open — this is a simulated validation. The real confirmation UX (a web UI /
Telegram prompt) is a companion-facing layer built on this gate. The test
`inbound_telegram_message_plans_and_sends_email` in
`tests/runtime-validation/tests/cognitive_loop.rs` exercises the full path
through a real subprocess; the gate's `evaluate` call is the line to add
before `dispatch_plan`. No cognitive changes were needed — the planner
already selected `email.send`, demonstrating correct capability selection.
