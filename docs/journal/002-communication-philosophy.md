# Behavior Journal

---

## Entry 002 — Communication Philosophy audit

**Date**: 2026-07-28

### Situation

The user defined a comprehensive Communication Philosophy: the companion must never expose internal implementation details in user-facing messages. Every greeting, notification, follow-up, signal description, and reason string was audited for leaks.

### Observation

The companion's messages contained systemic language that leaked internal architecture:

- **Greetings**: "I've noted you in my world model" — references World Model
- **Acknowledgements**: "Noted: {observation}" — parrots observations, robotic prefix
- **Continuity**: "Continuing where we left off: {obs}" — robotic prefix
- **Entity references**: "I see you're working on AI-OS (project)" — exposes entity types
- **Escalations**: "Escalation: {reason}" — system label prefix
- **Context summaries**: "No prior context — beginning fresh." — system-speak; "Recently: AI-OS (project)" — exposes types
- **Signal descriptions**: "person entities detected (strength=0.85)" — exposes internal metrics
- **Reason strings**: "combined signal strength 0.72 below threshold 0.30" — exposes internal metrics
- **Notification messages**: "Something deserves attention" — cold, system-like
- **Stagnation fallback**: "I noticed {name} ({type}) — anything new?" — exposes entity type

### Root cause

1. **cognitive_loop.rs** — the `decide()`, `decide_on_context()`, `stagnation_message()`, and `format_context_summary()` functions used implementation language directly in user-facing strings
2. **attention.rs** — the `evaluate()`, `evaluate_context()`, and `build_notification()` functions exposed strength values, signal names, and threshold logic

### Fix

Changed ~30 message templates across `cognitive_loop.rs` and `attention.rs`:

- All entity type annotations `({type})` removed from user-facing messages
- All strength values and threshold references removed from signal descriptions and reasons
- Robotic prefixes ("Noted:", "Continuing where we left off:", "Escalation:") replaced with natural language
- System-speak ("beginning fresh", "world model", "attention") replaced with companion-appropriate alternatives
- Notification messages warmed ("Something deserves attention" → "Something came up...")
- Signal descriptions simplified ("person entities detected (strength=0.85)" → "People are involved")

### Outcome

110 tests pass (91 unit + 12 integration + 7 simulation). 8 new behavioral tests verify:
- No internal terminology in any user-facing message path
- No strength values in signal descriptions
- No threshold/metric references in reason strings
- Clean notification messages
- Stagnation messages across all entity types

### Was it useful?

Yes. The companion will no longer sound like a system reporting its internal state. It will sound like a companion that simply understands.

### What should improve next?

- The runtime subsystem (`runtime.rs`, `event_driver.rs`) still uses internal terms like "goals", "retried", "blocked" in notification titles/messages. These are system-level notifications (not companion conversation), but they could be humanized in a future pass.
- The web UI status bar still shows "Entities: N" and "Relationships: N" — these are World Model metrics visible in the normal Status tab. They should either be moved to the Developer Dashboard or relabeled in more user-friendly terms.
