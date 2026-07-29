# Evaluation Framework Audit

## Purpose

Audit the Life Simulation & Evaluation Framework to determine whether every metric
actually measures the qualities expected from a Persistent Personal Intelligence.

**This is not an implementation task.** No production code changes are made here.
The goal is to evaluate whether the metrics themselves correctly measure companion quality.

---

## Background

After Stabilization Phase 1 refinements, the intelligence behaves more like a thoughtful
companion:

- Memory quality: 1.00 (was 0.50)
- Interruption quality: 0.70–0.77 (was 0.30)
- Overall evaluation score: decreased (because the metrics reward frequent communication)

This mismatch — behaviour improved but score decreased — reveals that the evaluation
framework itself needs auditing.

---

## Audit of Existing Metrics

### 1. Continuity — KEEP (but sharpen)

**What it measures:** Whether Communicate decisions mention entities from the current
context (entities in `build_context()` top 5).

**Does it measure genuine companion behavior?** Partially. A companion should reference
ongoing topics, but mentioning entity names is surface-level. Mentioning 5 entities by
name every tick scores 1.0 even if the communication is trivial ("Hey AI-OS, Rust,
Deployment — what's up?").

**Can it reward bad behavior?** Yes. Name-dropping all context entities produces a high
continuity score regardless of message quality. The AI gets full marks for a generic
greeting that happens to name the entities.

**Can it penalize good behavior?** No.

**Recommendation:** Keep the concept but raise the bar. A mention alone is not meaningful
continuity — the AI should demonstrate understanding of WHAT it's referencing (not just
naming it). However, this is hard to measure objectively in a simulation. Consider keeping
as-is with the understanding that it's a weak signal.

Weight: 0.25 → reduce to 0.15 in new scoring.

---

### 2. Relevance — KEEP (but sharpen)

**What it measures:** Whether cycle Communicate decisions contain non-generic language
("Noted" is treated as low-quality).

**Does it measure genuine companion behavior?** Partially. Distinguishing "I see you're
working on X" from "Noted" is useful. But any non-generic Communicate decision is scored
as relevant, even if the message is off-topic or repetitive.

**Can it reward bad behavior?** Mildly. Any Communicate ≠ "Noted" is scored as relevant.
The AI could say "I have no idea what you're doing but I'll say something anyway" and
score well on relevance.

**Can it penalize good behavior?** No.

**Recommendation:** Keep. The distinction between generic and non-generic responses is
measurable and meaningful. Consider adding a check for whether the response references
the specific entities or observations in the understanding, not just any non-generic text.

Weight: 0.20 → keep at 0.20 in new scoring.

---

### 3. Timeliness — REDEFINE or REMOVE

**What it measures:** Whether the AI communicates during silent periods when context exists.
Specifically: every silent day with context where the AI doesn't Communicate = "missed
check-in opportunity" (negative). Every silent day where the AI Communicates =
"timely check-in" (positive).

**Does it measure genuine companion behavior?** NO. This is the fundamental flaw of the
evaluation framework. The metric rewards communication frequency, not communication quality.
A companion that says something valuable every 5 days scores LESS than a companion that
says something trivial every day.

**Can it reward bad behavior?** YES, severely.

| Bad Behavior | Metric Reward |
|---|---|
| Spam: greeting every day regardless of context | +1.0 timeliness |
| Repeated reminders: "Checking in" on every silent tick | +1.0 timeliness |
| Unnecessary greetings: "Good morning. Recently: ..." when nothing changed | +1.0 timeliness |
| Constant follow-ups: asking about the same thing every tick | +1.0 timeliness |
| Excessive interruptions: communicating during every quiet moment | +1.0 timeliness |

**Can it penalize good behavior?** YES. Thoughtful silence — the AI has nothing meaningful
to say and stays quiet — is penalized as "missed check-in opportunity." This is the
opposite of the product philosophy.

**Root cause:** The metric conflates "time since last communication" with "appropriate
time to communicate." A good companion doesn't have a quota of things to say each day.

**Recommendation:** REMOVE as standalone metric. Replace with a composite judgment metric
(see new metrics below). The core insight is not wrong — an AI that never communicates
is also broken — but the current implementation punishes silence too harshly and rewards
activity too easily.

Weight: 0.15 → remove, distribute to new metrics below.

---

### 4. Adaptability — REDEFINE

**What it measures:** Entity turnover in context between consecutive days. If the context
entity on day N differs from day N+1, that's an "adaptability" point.

**Does it measure genuine companion behavior?** NO, inversely. High entity turnover means
the AI's context is unstable. A companion who is consistently focused on the same project
(AI-OS for 30 days) has LOW adaptability because the same AI-OS entity persists in
context every day. This is CORRECT companion behavior, but the metric penalizes it.

**Can it reward bad behavior?** YES. The AI gets high adaptability by:
- Following every new topic obsessively
- Never maintaining focus on an ongoing project
- Losing track of important entities when new ones appear

**Can it penalize good behavior?** YES. A companion with stable focus on important
projects (the desired behavior for continuity) is penalized for having consistent context.

**Root cause:** The metric measures "context instability" not "adaptability." True
adaptability is the ability to shift focus when priorities genuinely change — not a high
rotation rate of whichever entity is currently in context.

**Recommendation:** REDEFINE as "Focus Stability." Score should measure:
1. Does the AI maintain focus on high-importance entities over time? (positive signal)
2. When priorities genuinely shift (new entity enters, old entity leaves), does the AI
   notice and adjust within a reasonable timeframe? (positive signal)
3. Does the AI bounce between topics unnecessarily? (negative signal)

Weight: 0.15 → keep as "Focus Stability" at reduced weight (0.05).

---

### 5. Memory Quality — KEEP (but expand)

**What it measures:** Whether entities from day 1 are still in context on the final day.

**Does it measure genuine companion behavior?** YES, partially. A companion should remember
meaningful things. The correlation is good.

**Can it reward bad behavior?** Mildly. Storing every entity forever (even trivial ones)
inflates memory quality. Before our fix, reflection entities (importance 0.3) crowded the
top-5 context and artificially inflated their own persistence metrics. This was fixed.

**Can it penalize good behavior?** Not directly. But the metric doesn't distinguish between
"remembering important things" and "remembering trivial things."

**Recommendation:** KEEP. Consider adding importance-weighted scoring where high-importance
entities matter more than low-importance ones for the persistence check. But the current
implementation is acceptable.

Weight: 0.15 → keep at 0.10 in new scoring (importance-weighted).

---

### 6. Interruption Quality — REDEFINE as Attention Cost

**What it measures:** Communication cost during silent periods (no user observation).
Specifically: the ratio of AI Communicates during silent days to total silent days. Lower
is better.

**Does it measure genuine companion behavior?** Partially. It correctly penalizes nagging.
But it also penalizes necessary communication — checking in about a stagnant project
during a 4-day silent period is a good thing, not an interruption.

**Can it reward bad behavior?** YES. An AI that never communicates scores 1.0 interruption
quality. A completely silent AI is a broken AI.

**Can it penalize good behavior?** YES. A timely follow-up about a stagnating project
during silence is correctly scored as an "interruption," reducing the metric.

**Root cause:** The metric treats ALL communication during silence as cost, but some
communication during silence is high-value (stagnation follow-up, acknowledging progress,
noticing a problem).

**Recommendation:** REDEFINE as "Attention Cost" with a cost-benefit ratio:
- Cost of communication during silence = interruption penalty
- Benefit of communication during silence = stagnation detected, high-importance change,
  meaningful follow-up
- Score = 1.0 - (cost * 0.5 - benefit * 0.5), clamped to [0, 1]

Weight: 0.10 → keep at 0.10 in new scoring (as Attention Cost).

---

## New Evaluation Philosophy

### Core Principle: Value over Activity

The AI should be evaluated on the **quality and value** of its communications, not on
**how frequently** it communicates. A thoughtful companion says the right thing at the
right time, and knows when to be silent.

### Why Activity-Based Metrics Are Wrong

The old timeliness metric (weight: 0.15) rewarded the AI for communicating every day
when context existed. This produces a companion that:
1. Says "Good morning. Recently: X, Y, Z." every day regardless of whether anything
   meaningful happened since the last communication.
2. Gradually becomes notification fatigue (same pattern every tick).
3. Feels like checking in with a colleague who has nothing new to say but says it
   anyway to stay "in the loop."

The product philosophy explicitly prefers **thoughtful communication** and **appropriate
silence**. The metrics should reinforce this.

### New Scoring Philosophy

| Dimension | Weight | Measures |
|---|---|---|
| Trust | 0.20 | Does the AI's behaviour feel consistent and reliable? |
| Usefulness | 0.25 | Does communication add value (new insight, acknowledgment, follow-up)? |
| Judgment | 0.20 | Does the AI know when to speak and when to be silent? |
| Context Awareness | 0.15 | Does the AI reference and maintain ongoing context? |
| Attention Cost | 0.10 | What is the cost of the AI's interruptions? |
| Memory Quality | 0.10 | Does the AI persist meaningful entities over time? |
| Total | 1.00 | |

### New Metric: Trust (0.20 weight)

**Rationale:** A trusted companion behaves predictably. It doesn't surprise you with
random check-ins, and it doesn't go silent when something matters. Trust is built over
time through consistent, appropriate behaviour.

**Measurement:**
- Ratio of Communicate decisions that are followed by positive user interaction (next
  cycle() with non-generic input referencing the AI's previous message)
- Penalty for consecutive Communicate decisions on the same topic (nagging detection)
- Bonus for Communicate decisions when there's a genuine stagnation or change signal

### New Metric: Usefulness (0.25 weight, highest)

**Rationale:** The most important quality of a companion is whether its communication
adds value. A message that provides new insight, acknowledges progress, or follows up
on something meaningful is useful. A generic greeting is not.

**Measurement:**
- Does the Communicate message reference a specific entity, observation, or change?
  (not just "Recently: X, Y, Z")
- Does the decision to Communicate coincide with a genuine attention signal?
  (stagnation, progress, regression — not just "context exists")
- Does the message advance the conversation or acknowledge meaningful progress?

### New Metric: Judgment (0.20 weight)

**Rationale:** Knowing WHEN to communicate is as important as knowing WHAT to say.
Good judgment means the AI stays silent when silence is appropriate and speaks when
something matters.

**Measurement:**
- Does the AI Communicate only when there's a genuine justification (stagnation > 0.15,
  high importance + sustained silence, explicit user request)?
- Does the AI stay silent (return Wait) when context exists but there's no meaningful
  reason to reach out?
- Is the timing appropriate? (not first-tick panic, not 30-day silence, but measured
  check-ins about things that need attention)

### New Metric: Context Awareness (0.15 weight)

**Rationale:** A companion should know what's been happening and maintain continuity.
This measures whether the AI's responses are informed by ongoing context.

**Measurement (KEEP existing continuity metric, sharpened):**
- Does the Communicate message reference specific context entities with substance?
  (not just name-dropping)
- Does the AI maintain awareness of important entities over multi-day gaps?
- Does the AI connect current observations to prior context?

### New Metric: Attention Cost (0.10 weight)

**Rationale:** Every communication has a cost (interruption). The AI should minimize
unnecessary interruptions while staying engaged enough to notice when something matters.

**Measurement (REDEFINE from interruption quality):**
- Cost of each Communicate: 1 point per interruption during a silent streak
- Benefit of each Communicate: +2 points if there's a genuine justification
  (stagnation, regression, progress), +1 point if there's mild justification
- Score = max(0, 1.0 - (total_cost - total_benefit) / max(1, total_opportunities))

### New Metric: Long-Term Memory (0.10 weight)

**Rationale:** A companion should remember what matters and let go of what doesn't.
Current memory quality measures persistence of day-1 entities through the final day.
This is good but should be importance-weighted.

**Measurement (ENHANCE existing memory quality metric):**
- Average importance of retained entities vs average importance of lost entities
- High importance entities retained = higher score
- Low importance entities naturally dropped = lower penalty (healthy forgetting)

---

## Metrics to Keep vs Redefine vs Remove

### Keep (with enhancements)
| Metric | Current Weight | New Weight | Change |
|---|---|---|---|
| Memory Quality | 0.15 | 0.10 | Reduce; add importance weighting |
| Relevance | 0.20 | 0.25 | Increase; this is the most meaningful measurable signal |
| Focus Stability (was Adaptability) | 0.15 | 0.05 | Reduce; this was measuring instability, not adaptability |

### Redefine
| Metric | Current Name | New Name | Change |
|---|---|---|---|
| Timeliness | Timeliness | (merged into) Judgment | Replace entirely with new judgment metric |
| Interruption Quality | Interruption Quality | Attention Cost | Redefine with cost-benefit analysis |
| Continuity | Continuity | Context Awareness | Keep concept, sharpen measurement |

### Remove
| Metric | Reason |
|---|---|
| Adaptability (old) | Measures context instability, not true adaptability. Replaced by Focus Stability. |

---

## Updated Companion Score Definition

```
companion_score = 0.20 * Trust
                + 0.25 * Usefulness
                + 0.20 * Judgment
                + 0.15 * Context_Awareness
                + 0.10 * Attention_Cost
                + 0.10 * Long_Term_Memory
```

A score of 1.0 means: the AI is trusted, useful, shows good judgment, maintains context
awareness, has low attention cost, and has good long-term memory.

### Why This Scoring Philosophy Is Better

In the new framework, an AI that:
- Communicates only when something genuinely needs attention → HIGH (judgment + usefulness)
- Stays silent when there's nothing meaningful to say → HIGH (judgment, low attention cost)
- Remembers important entities across weeks → HIGH (long-term memory)
- Maintains awareness of ongoing topics → HIGH (context awareness)
- Behaves consistently over time → HIGH (trust)

And an AI that:
- Sends a generic greeting every day → LOW (low usefulness + high attention cost)
- Never communicates even when something matters → LOW (no usefulness, low trust)
- Bounces between topics daily → LOW (low focus stability, low trust)
- Talks repeatedly about the same thing → LOW (high attention cost, low trust)

This aligns with the product philosophy: **thoughtful companion, not notification system.**

---

## Simulation Review: Would Current Framework Reward Bad Behavior?

Yes, the old framework would reward all of the following undesirable behaviours:

### Spam
A companion that sends "Good morning. Recently: Nothing." every day (even when the
user hasn't said anything meaningful) scores 1.0 on timeliness and 1.0 on continuity.
**The metric rewards activity.**

### Repeated Reminders
A companion that follows up on the same stagnating entity every tick (stagnation > 0.3)
scores 1.0 on timeliness. The user gets notified about the same thing repeatedly.
**The metric doesn't detect nagging.**

### Unnecessary Greetings
A companion that greets the user every tick with "Good morning. Recently: X, Y, Z."
scores 1.0 on continuity and 1.0 on timeliness. The messages are generic and add no value.
**The metric rewards verbosity.**

### Constant Follow-ups
A companion that follows up on every entity in context every tick scores 1.0 on all
context-based metrics. The user is overwhelmed with "I noticed you were working on: A, B,
C. What about D? Did you finish E?" every day.
**The metric rewards over-communication.**

### Excessive Interruptions
An AI that communicates on every tick regardless of activity level scores perfectly on
timeliness and loses zero on interruption quality (because the interruption quality
penalty only applies during silent streaks).
**The metric doesn't penalize excessive communication during active periods.**

All of these behaviours score well under the old evaluation framework because the metrics
measure activity frequency, not communication value.

---

## Summary

### Key Finding
The evaluation framework measures **activity** (communication frequency) not **value**
(quality of communication). This systematically rewards the wrong behavior:
- Over-communication is rewarded
- Thoughtful silence is penalized
- Nagging and repetition are not detected as negative

### Fix Strategy
1. Remove the Timeliness metric (rewarding communication frequency)
2. Redefine Interruption Quality as Attention Cost (cost-benefit analysis)
3. Redefine Adaptability as Focus Stability (measuring stable attention on important work)
4. Add Trust, Usefulness, and Judgment as primary evaluation dimensions
5. Weight judgments toward Usefulness (0.25) and Judgment (0.20) — the qualities that
   define a trusted companion

### No Production Code Changes
This audit recommends changes to the evaluation framework only, not to the intelligence
core itself. The intelligence refinements from Stabilization Phase 1 should be preserved.

### Next Steps
1. Implement the new evaluation philosophy (new metrics, new weight distribution)
2. Re-run all 6 scenarios under the new framework
3. Compare scores to verify that the new framework correctly rewards thoughtful behaviour
4. Run real user studies to validate the new dimensions