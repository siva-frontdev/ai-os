# Behavior Journal

Entries follow the real-world improvement loop: observe, trace root cause, fix minimally, verify.

---

## Entry 001 — First greeting too interview-like

**Date**: 2026-07-28

### Situation

The companion's first interaction with its first real user. The user introduced the mission: the companion should now behave like a daily-life companion, not a software project. The companion responded by asking "Tell me something about your life — what does a typical day look like for you right now?"

### Observation

The companion asked for the user's entire life story in one go. The question was framed as a broad interview prompt rather than a gradual, trust-earning conversation opener.

### Understanding

The companion detected a person/user entity with no prior context (empty World Model). The `decide()` method in `cognitive_loop.rs:412-413` produced:

> "Hello {name}! I've noted you in my world model."

This message:
- References internal system terminology ("world model") — breaks the companion illusion
- Implies the user is being catalogued, not met
- Sets a transactional tone rather than a relational one

### Decision to communicate

The `decide()` method matched the first entity as type "person"/"user" with `has_continuity = false`. The greeting was hardcoded, not LLM-generated. The companion had no choice of tone — it was forced into system-speak by the code.

### Outcome

The user explicitly rejected the framing. They gave detailed feedback about what a better first interaction should look like:
- Start with what's relevant today, not the entire life
- Build understanding gradually through shared experience
- End with something like "We don't need to build your world today. We'll build it together, one conversation at a time."

### Root cause

`cognitive_loop.rs:412-413` — the first-user greeting string in `CognitiveLoopService::decide()`.

```
format!("Hello {}! I've noted you in my world model.", entity.name)
```

### Fix

Changed the first-user greeting to embody the gradual trust philosophy:

```
"I don't need to understand your whole life today — we'll build that together over time. What's on your mind right now?"
```

Also softened the non-person first greeting from:

```
"I see you're working on {name} ({type})."
→
"I noticed you're working on {name} ({type}). What's the latest?"
```

And the returning non-person greeting from:

```
"I see you're working on {name} ({type}). {context_summary}"
→
"I noticed you're still working on {name} ({type}). {context_summary}"
```

### Subsystem

`cognitive_loop.rs` — `CognitiveLoopService::decide()` (communication decision layer).

### Simulation update

No simulation update needed — the greeting messages are not tested for exact content, only for decision type. All 102 existing tests pass unchanged.

### Was it useful?

Yes. The user's feedback revealed a gap between the companion philosophy (gradual trust, earned understanding) and the actual code (system-language greetings). The fix aligns the code with the philosophy.

### What should improve next?

- The companion currently has **no persona system prompt** — its voice is entirely hardcoded in Rust strings. If the companion is to feel like a genuine companion, the communication layer should eventually use a configurable persona prompt rather than hardcoded templates.
- However, that would require new prompt infrastructure, which violates "no new engines." For now, refining the hardcoded strings is the right approach.
