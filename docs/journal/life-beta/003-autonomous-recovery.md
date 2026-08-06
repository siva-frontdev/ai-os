# Experience Journal: LIFE Beta

---

## Day 003 — Autonomous goals can't be resumed mid-execution

**Date**: 2026-08-06

### What happened

I spent the day validating LIFE's autonomous layer end to end
(`tests/phase6-autonomous`): goal sessions, cognitive ticks, plan repair,
approval gating, multi-agent delegation, recovery, and learning. This was
real use of the frozen architecture — driving the real `BrainOrchestrator`,
`GoalManager`, `AgentRegistry`, `RecoveryManager`, `PolicyRegistry`,
`Reflector`, and `LearningEngine` with deterministic stubs.

During recovery testing, LIFE exposed a genuine behavioral defect. The
`RecoveryManager` is documented to "automatically resume all interrupted
goals" and classifies any goal still in `Active` / `Planning` / `Executing` /
`Evaluating` as interrupted (these are the goals that were mid-flight when a
restart happened). But when `resume_all` called `recover_goal`, the goal
manager rejected the transition:

```
[recovery] failed to resume goal 019fd55d-...: validation error:
goal cannot be recovered from status Active
```

A goal that was actively running at restart — precisely the goal that most
needs recovery — could not be resumed. Only already-`Failed` or already-
`Recovering` goals were accepted.

### What LIFE did well

- The autonomous loop (cognitive tick → strategy assignment → objective
  decomposition) ran correctly against the real brain layer.
- Capability selection was verified: the right agent was chosen for the right
  task, high-risk capabilities required confirmation, and denied capabilities
  could not be executed by anyone.
- Reflection → learning → strategy feedback closed the loop correctly after
  repeated planning mistakes.
- `recover_goal` surfaced a clear, typed error rather than silently
  misbehaving — the fail-fast principle held.

### What LIFE should have done

- Resume a goal that was mid-execution at restart. The whole point of
  recovery is to pick up interrupted work; an `Active` goal is the canonical
  interruption, and LIFE refused to recover it.

### Root cause

- **Judgment** — the goal state machine's recovery precondition did not match
  the recovery manager's definition of "interrupted." `recover_goal` in
  `brain/brain-goals/src/manager.rs` only accepted `Failed` / `Recovering`,
  while `RecoveryManager::find_interrupted_goals` returns `Active` /
  `Planning` / `Executing` / `Evaluating` / `Recovering`. The two contracts
  disagreed, so the documented behavior ("resume interrupted goals") was
  impossible for the most common case.

### Proposed improvement

- Widen `recover_goal` to accept every interrupted state the recovery manager
  recognizes: `Active`, `Planning`, `Executing`, `Evaluating`, `Failed`, and
  `Recovering`. This makes `resume_all` actually resume mid-execution goals
  and marks them `Recovering` so the next cognitive tick re-plans and
  continues them. No architectural change — a one-method contract fix inside
  the frozen Goal Manager. Applied in `brain/brain-goals/src/manager.rs`;
  verified by `resume_all_recovers_interrupted_goals` and the full
  brain-goals + phase6-autonomous suites.
