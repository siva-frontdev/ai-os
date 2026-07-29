# ADR-0007: Continuous Cognitive Loop

## Status

Accepted

## Date

2026-07-27

## Context

### The Question ADR-0006 Left Open

ADR-0006 established the **World Model** — a continuously evolving property graph of everything the system understands about the user's world. It answered the question "what do I know?" — entities, relationships, importance, confidence, and the dynamic subgraph of context.

But the World Model alone is static. It is a body of knowledge without a mind to think about it. A perfectly maintained graph of entities and relationships is still just a database until something continuously observes, interprets, reflects, predicts, decides, and learns from it.

This ADR answers the question that ADR-0006 left open:

> "Given everything I know, how do I continuously think?"

### Why Continuous?

The AI must not wait for prompts. A prompted system is a chatbot — it thinks only when asked, forgets everything between turns, and has no persistent understanding. The AI must behave like an intelligent companion that is always present, always aware, always learning, and always deciding whether to remain silent or speak.

The difference is fundamental:

| Aspect | Prompted System | Continuous Intelligence |
|---|---|---|
| When does it think? | When asked | Always |
| What does it remember? | Conversation history | Understanding extracted from all interactions |
| When does it learn? | Never (stateless) | Every cycle |
| When does it act? | When commanded | When reasoning determines it is appropriate |
| When does it speak? | When prompted | When it has something worth saying |
| When does it stay silent? | When not prompted | By default — silence is the baseline |

### What This ADR Is Not

This ADR is not a planning algorithm. It does not define how to decompose goals into subgoals, how to search a plan space, or how to optimize resource allocation. Those are implementation concerns for later phases.

This ADR is not an architecture layer. It does not introduce new crates, new daemons, new services, or new runtime components. The existing layered architecture — Core, Runtime, Memory, Brain, Perception, Execution, OSAL, Intelligence — remains unchanged.

This ADR is not a workflow. It does not define a pipeline of stages connected by queues. It describes cognition — a continuous, recursive, self-referential process that has no beginning and no end.

### Relationship to the Existing Brain

The existing Brain Platform architecture defines a `CognitiveLoop` with a `tick()` method. That loop was designed around goals: a goal arrives, the Brain plans, reasons, decides, executes, reflects, learns, and returns to idle.

This ADR reframes that loop. It is no longer a goal-driven cycle that runs when work is available. It is a continuous cognitive process that is always active — not in the sense of consuming CPU cycles, but in the sense that the system is always ready to perceive, always ready to reflect, always ready to learn. The `tick()` becomes a heartbeat, not a work unit.

The implementation of the Brain's coordinator crate does not change. What changes is the philosophy of when and why the cognitive process runs — not on demand, but continuously.

### The Core Insight

The difference between a tool and an intelligence is not capability. It is **initiative**.

A tool waits to be used. An intelligence continuously considers whether to act.

A tool responds. An intelligence reflects before responding — and often decides that no response is needed.

A tool forgets everything after the interaction. An intelligence integrates every interaction into a growing understanding.

This ADR defines how the AI develops and exercises initiative — not as a feature, but as the fundamental mode of operation.

## Decision

We adopt a **Continuous Cognitive Loop** as the defining behaviour of the Persistent Personal Intelligence. This is not a component, not a service, not an algorithm. It is the philosophical foundation for how the AI thinks.

### Core Principle

The AI never waits for a prompt. It continuously cycles through observation, interpretation, reflection, prediction, prioritisation, decision, communication, execution, and learning. Every cycle updates its understanding. Every cycle makes it slightly better at understanding the user's world.

The AI never asks "what command did the user give?" It continuously asks:

> What changed? Why did it change? What does it affect? What relationships changed? What opportunities appeared? What risks appeared? What assumptions became invalid? Should I remember this? Should I forget something? Should I ask? Should I recommend? Should I wait? Should I act? What did I learn?

---

## The Cognitive Philosophy

### Thinking Is Not Responding

A chatbot thinks only to respond. Its cognitive process is: receive message → process → generate reply → done. The user's message is the start and the reply is the end.

The continuous intelligence thinks regardless of input. Its cognitive process is a closed loop that never stops. Observations enter the loop at any point. Decisions exit the loop at any point. But the loop itself has no beginning and no end.

The consequence is profound: the AI does not need the user to initiate interaction. It initiates interaction when its reasoning determines that doing so serves the user's interests better than remaining silent.

### Silence Is the Default

The most important decision the AI makes is whether to communicate. The default is silence. Every cognitive cycle may produce a decision, but most decisions are "do nothing." The AI must actively justify every interruption, every suggestion, every question.

This is the opposite of a chatbot, which is silent until prompted and then always responds. The continuous intelligence is always thinking but almost always silent. It speaks only when:
- The user asks a direct question
- It detects a situation that warrants attention
- Its confidence is high and the information is timely
- The cost of silence exceeds the cost of interruption

### Understanding Is the Goal

The AI has no "goals" in the traditional sense. It does not pursue objectives. It pursues understanding. Everything feeds into the World Model. Every observation, every interaction, every reflection is an opportunity to deepen understanding.

From understanding, everything else emerges:
- Good predictions emerge from accurate understanding
- Good decisions emerge from accurate predictions
- Good communication emerges from accurate decisions about what matters

The AI optimises for one thing: the accuracy and depth of its World Model. Everything else is downstream.

---

## The Cognitive Loop

### Structure

The cognitive loop is a continuous, recursive, self-referential cycle. It has no beginning and no end. It is described here as a sequence for comprehension, but in reality all phases overlap and feed into each other:

```
                        ┌──────────────────────────┐
                        │       OBSERVE             │
                        │  (Perception layer)       │
                        │  Raw input from any       │
                        │  interface becomes        │
                        │  entities & relationships │
                        └──────────┬───────────────┘
                                   │
                                   ▼
                        ┌──────────────────────────┐
                        │      INTERPRET            │
                        │  What does this mean?     │
                        │  What entities exist?     │
                        │  What relationships?      │
                        │  What is the confidence?  │
                        └──────────┬───────────────┘
                                   │
                                   ▼
                        ┌──────────────────────────┐
                        │    UPDATE WORLD MODEL     │
                        │  (Memory layer)           │
                        │  Insert or update         │
                        │  entities & relationships │
                        │  Recompute importance     │
                        │  Emit change events       │
                        └──────────┬───────────────┘
                                   │
              ┌────────────────────┼────────────────────┐
              │                    │                    │
              ▼                    ▼                    ▼
   ┌──────────────────┐  ┌──────────────────┐  ┌──────────────────┐
   │    REFLECT        │  │    PREDICT        │  │    GENERATE       │
   │  What changed?    │  │  What will happen?│  │    CONTEXT        │
   │  Why did it       │  │  What deadlines   │  │  Dynamic subgraph │
   │  change?          │  │  are approaching? │  │  from seeds       │
   │  What is affected?│  │  What risks are   │  │                   │
   │  What should I    │  │  forming?         │  │                   │
   │  reconsider?      │  │  What confidence? │  │                   │
   └────────┬─────────┘  └────────┬─────────┘  └────────┬─────────┘
            │                     │                      │
            └─────────────────────┼──────────────────────┘
                                  │
                                  ▼
                    ┌──────────────────────────┐
                    │    PRIORITISE             │
                    │  What matters most        │
                    │  right now?               │
                    │  What should I focus      │
                    │  this cycle on?           │
                    └──────────┬───────────────┘
                               │
                               ▼
                    ┌──────────────────────────┐
                    │      DECIDE               │
                    │  Exactly one decision:    │
                    │  • Do nothing             │
                    │  • Wait                   │
                    │  • Observe again          │
                    │  • Ask                    │
                    │  • Suggest                │
                    │  • Notify                 │
                    │  • Learn                  │
                    │  • Update memory          │
                    │  • Create hypothesis      │
                    │  • Request permission     │
                    │  • Execute action         │
                    └──────────┬───────────────┘
                               │
                    ┌──────────┴──────────┐
                    │                     │
                    ▼                     ▼
         ┌──────────────────┐  ┌──────────────────┐
         │  COMMUNICATE      │  │  EXECUTE          │
         │  (if decision is  │  │  (if decision is  │
         │  ask/suggest/     │  │  execute)         │
         │  notify)          │  │  Performs action  │
         │  Via any interface│  │  via Execution    │
         └────────┬─────────┘  │  layer             │
                  │            └────────┬─────────┘
                  │                     │
                  └──────────┬──────────┘
                             │
                             ▼
                    ┌──────────────────────────┐
                    │    OBSERVE RESULT         │
                    │  What happened?           │
                    │  Was it expected?         │
                    │  What changed in the      │
                    │  world?                   │
                    └──────────┬───────────────┘
                               │
                               ▼
                    ┌──────────────────────────┐
                    │      LEARN                │
                    │  Update confidence        │
                    │  Adjust predictions       │
                    │  Refine understanding     │
                    │  Improve timing           │
                    │  Update World Model       │
                    └──────────┬───────────────┘
                               │
                               └──────→ BACK TO OBSERVE
```

### How the Loop Runs

The loop is not a busy-wait. It is event-driven with a background heartbeat:

**Triggered cycles** — When a significant event occurs (new observation from Perception, World Model change, user interaction), the loop cycles immediately. The trigger is the event itself, not a timer.

**Heartbeat cycles** — When nothing significant has occurred, the loop cycles on a configurable interval (default: approximately 1 second). This ensures the AI continuously reflects even in the absence of new input. Time passing is itself a change worth reflecting on.

**Depth of cycles** — Not every cycle is equal. Most heartbeat cycles are shallow: generate context, check for changes, decide "do nothing," learn nothing new, cycle again. Triggered cycles are deep: full interpretation, reflection, prediction, and potential action. The depth of a cycle is proportional to the significance of what triggered it.

### Relationship Between Phases

The phases of the cognitive loop are not sequential stages in a pipeline. They are facets of a single cognitive act that the description artificially separates:

- **Observe and Interpret** are the same act. The AI does not first see raw data and then understand it. It understands as it observes. Perception and interpretation are one continuous process.

- **Reflect and Predict** feed each other. Reflection on the past informs prediction of the future. Prediction reveals what to reflect on. They are two directions of the same cognitive movement.

- **Prioritise and Decide** are inseparable. Prioritisation is the process of deciding what matters. Decision is the outcome of that process. They are not two steps but one.

- **Communicate and Execute** are both forms of action. The AI acts either by communicating with the user or by executing an action in the world. Both change the world and produce new observations for the next cycle.

- **Observe Result and Learn** are the same act. Observing the result of an action is learning. The AI cannot observe without learning, and it cannot learn without observing.

---

## Reflection Model

### What Reflection Is

Reflection is the act of considering the current state of the World Model and asking what it means. It is not triggered only by user prompts. It is a continuous background process that runs on every cognitive cycle.

### What Triggers Reflection

Reflection is triggered by any change in the World Model or the passage of time:

| Trigger | Why It Matters |
|---|---|
| New observation | The world has new information. What does it mean? |
| Time passing | The world changed while the AI was not looking. What may have changed? |
| Environment change | System state changed. What are the implications? |
| Project change | A project entity was updated. What is affected? |
| Relationship change | A relationship was added or removed. What does this connect? |
| Learning progress | The AI's understanding improved. What else should be reconsidered? |
| Unexpected event | An anomaly was detected. What caused it? |
| Routine change | A habitual pattern was broken. Why? |
| Approaching threshold | A deadline, limit, or milestone is near. Is the user prepared? |
| Prediction failure | What the AI expected did not happen. What was wrong? |
| Long time since reflection | The AI has not deeply reflected in a while. It should. |

### What Reflection Considers

Every reflection considers:

1. **What changed?** — The difference between the current World Model and the previous cycle's snapshot. Not just new entities, but changes in properties, relationships, importance, and confidence.

2. **Why did it change?** — The inferred cause of each change. Was it user action? System event? External factor? Confidence decay? Time passage?

3. **What does it affect?** — The ripple effect through the graph. Which entities are connected to the changed entities? What paths exist between the change and the user's important concerns?

4. **What relationships changed?** — New relationships, removed relationships, changed weights and confidence. Relationship changes often matter more than entity changes because they reveal new connections or broken ones.

5. **What opportunities appeared?** — New configurations of entities and relationships that could benefit the user. A new contact connected to a project. A new tool that could solve a known problem.

6. **What risks appeared?** — Configurations that could harm the user's interests. A blocked dependency. A neglected relationship. A deadline approaching without progress. Declining health metrics.

7. **What assumptions became invalid?** — Previous beliefs that the new information contradicts. The AI held an assumption with some confidence; new evidence lowers that confidence. What else depended on that assumption?

8. **What should I remember?** — Entities or relationships that are important but near the archival threshold. Patterns that have repeated enough to warrant consolidation.

9. **What should I forget?** — Entities or relationships whose confidence has decayed below usefulness. Information that turned out to be wrong.

### Depth of Reflection

Reflection has depth, proportional to the significance of the trigger:

| Depth | Trigger | Cost | Outcome |
|---|---|---|---|
| Shallow | Heartbeat tick, minor update | Low (sub-ms) | Quick check: anything urgent? If not, "do nothing" |
| Medium | Entity or relationship change | Moderate (10-100ms) | Full analysis of change, immediate ripple effects |
| Deep | Significant event, anomaly, user correction | High (100ms-2s) | Full graph analysis, hypothesis generation, model call if needed |
| Profound | Rare: major world change, long absence | Highest (2-10s) | Full cognitive cycle with deep learning and model-based reasoning |

The AI manages its cognitive budget by choosing the appropriate depth for each triggered cycle. Most cycles are shallow. Few are profound.

---

## Decision Model

### One Decision Per Cycle

Every reflection produces exactly one decision. The decision may be "do nothing" — and most reflections produce that decision — but the system must explicitly decide, not implicitly default.

### Possible Decisions

| Decision | Meaning | When |
|---|---|---|
| Do nothing | The world is stable. No action needed. | Default. Most common decision. |
| Wait | Something might change soon. Monitor. | Confidence is low. More information expected. Consequences of premature action are significant. |
| Observe again | The observation was ambiguous. Get more data. | Entity resolution had low confidence. Need another data point. |
| Ask a question | User input is needed. | Low confidence on critical entity or relationship. User is the authority. |
| Suggest something | User might benefit from this. | High confidence opportunity identified. Low interruption cost. |
| Notify | User needs to know this. | Time-sensitive situation. Risk or opportunity warrants attention. |
| Learn | Update understanding without user involvement. | Pattern detected. Confidence adjustment needed. Entity consolidation. |
| Update memory | The World Model needs structural change. | Entity resolution, relationship inference, archival, forgetting. |
| Create hypothesis | A possible explanation or prediction. | Anomaly detected. Pattern observed. Uncertain causality. |
| Request permission | Action is needed but not authorized. | High-value action outside automated permission scope. |
| Execute action | Act on the world. | Clear, high-confidence, in-scope action. User benefit is clear. |

### Decision Criteria

The AI evaluates each possible decision against these criteria:

| Criterion | What It Measures |
|---|---|
| Importance | How central is this to the user's world? (0.0 - 1.0) |
| Urgency | How time-sensitive is this? (0.0 - 1.0) |
| Impact | What is the expected value of acting? (negative to positive) |
| Confidence | How sure are we about our understanding? (0.0 - 1.0) |
| User preference | Has the user indicated preference in similar situations? |
| Interruption cost | What is the cost of interrupting the user? |
| Irreversibility | Can this be undone if wrong? |

### The Decision Heuristic

The AI uses (but is not limited by) this heuristic to select a decision:

```
if urgency > threshold(critical) and confidence > threshold(high):
    → Execute or Notify (interrupt)

if importance > threshold(high) and confidence > threshold(high):
    → Suggest or Ask (non-interrupting)

if importance > threshold(medium) and urgency > threshold(medium):
    → Notify (subtle, non-blocking)

if anomaly_detected and confidence > threshold(medium):
    → Create hypothesis, then Observe again

if confidence < threshold(low) for critical entity:
    → Observe again or Ask

if opportunity_detected and confidence > threshold(high):
    → Suggest

if no significant change and no opportunity and no risk:
    → Do nothing

if confidence_decay_threshold_reached:
    → Update memory (reconsolidate or forget)

if all else falls through:
    → Do nothing
```

The heuristic is not rigid. It evolves through learning. What the user ignores, the AI stops suggesting. What the user appreciates, the AI does more. The decision model learns from every outcome.

---

## Prediction Model

### Continuous Prediction

The AI continuously predicts the future state of the World Model. Prediction is not a separate process that runs on a schedule. It is an integral part of every cognitive cycle.

Every prediction has:
- **Subject** — the entity or relationship being predicted
- **Predicted state** — the expected future state or value
- **Time horizon** — when the prediction applies (in 1 hour, tomorrow, next week)
- **Confidence** — how sure the AI is (0.0 - 1.0)
- **Evidence** — what the prediction is based on

### What the AI Predicts

| Prediction | Example | Horizon |
|---|---|---|
| Entity state | "Project X will be completed" | Days to months |
| Relationship change | "User will stop using tool Y" | Weeks to months |
| Deadline outcome | "User will miss the deadline for Z" | Days to weeks |
| Blocker emergence | "Dependency A will block task B" | Hours to days |
| Health trend | "User's step count will decline this week" | Days |
| Learning gap | "User will struggle with concept C" | Variable |
| Relationship neglect | "User has not contacted D in 30 days" | Rolling window |
| Missed opportunity | "Conference E would benefit project F" | Variable |
| Routine interruption | "User will skip morning run due to weather" | Hours to days |
| User state | "User is entering focus mode" | Minutes to hours |

### How Predictions Are Made

Predictions are formed by:
1. **Pattern matching** — The AI recognises that the current state resembles previous states, and projects the trajectory forward. This requires the World Model to have stored the historical patterns.

2. **Trend extrapolation** — Entity properties that change linearly or cyclically are projected forward. Step count declining → will reach low threshold in N days.

3. **Graph traversal** — If entity A depends on entity B, and entity B is predicted to fail, then entity A is predicted to be blocked.

4. **Model-based reasoning** — When patterns and trends are insufficient, the AI uses the Intelligence layer (LLM) to generate predictions from natural language understanding of the situation.

### Prediction Lifecycle

```
1. Prediction formed (confidence: C_initial)
2. Prediction stored as entity with type "prediction"
3. Related to predicted entity via "predicts" relationship
4. As time passes, prediction confidence updates:
   - Evidence confirms → confidence increases
   - Evidence contradicts → confidence decreases
   - Time passes without event → confidence changes based on prediction type
5. When prediction horizon arrives:
   - Evaluate accuracy → update confidence and learn
   - If accurate → reinforce prediction patterns
   - If inaccurate → weaken prediction patterns, investigate why
6. If confidence drops below threshold → prediction is archived
```

### Prediction Entities in the World Model

Predictions are entities:

```
Entity: Prediction-2026-07-27-001
  entity_type: "prediction"
  properties:
    predicted_state: "blocked"
    horizon: "2026-08-01"
    confidence: 0.72
    evidence: ["dependency A has failing CI", "dependency A has no recent commits"]
    status: "active"  # active, confirmed, refuted, expired
  relationships:
    - type: "predicts" → Task("Upgrade dependency A")
    - type: "derived_from" → Observation("CI failure on A")
    - type: "affects" → Project("AI-OS")
```

---

## Prioritisation Model

### Continuous Reprioritisation

Prioritisation is never a single event. It is a continuous re-evaluation that happens on every cognitive cycle. The AI does not maintain a priority queue. It evaluates priority dynamically based on the current state of the World Model.

### Factors

| Factor | What It Measures | How It Changes |
|---|---|---|
| Entity importance | How central is this entity to the user's world? | Computed from graph centrality; updates as the graph changes |
| Urgency | Is there a time constraint? | Based on deadlines, approaching thresholds, scheduled events |
| Impact | What is the magnitude of the potential outcome? | Based on graph reach — how many entities are affected |
| Confidence | How sure are we about our understanding? | Updates with new evidence; decays without |
| User preference | What has the user shown they care about? | Learned from user behaviour over time |
| Long-term goal | What direction is the user moving? | Goal entities in the World Model with their deadlines |
| Current context | What is relevant right now? | Dynamic subgraph from the context generator |
| User energy | Is the user available and receptive? | Learned from user activity patterns, calendar, focus mode |
| Time available | How much time does the user have? | Calendar entities, time of day, typical patterns |
| Recency | How recently was this entity relevant? | Decay from last interaction |
| Prediction confidence | How sure is the prediction? | Lower confidence predictions have lower priority |
| Anomaly score | Is this unusual? | High anomaly scores bypass normal prioritisation |

### How Prioritisation Works

Prioritisation is not a formula. It is a learned weighting of factors that evolves with the user. The AI learns which factors matter most for which types of situations.

Conceptually:

```
priority = Σ(wi × fi)

where:
  wi = learned weight for factor i
  fi = normalized value for factor i (0.0 - 1.0)
```

The weights are not static. They evolve through learning:
- User responds to a notification → weight for that notification type increases
- User ignores a suggestion → weight for that suggestion type decreases
- User cancels an action → weight for urgency decreases, weight for confirmation increases

### What Gets Prioritised

The prioritisation model answers one question: "What should this cognitive cycle focus on?"

The answer is always an entity or relationship in the World Model. The highest-priority entity becomes the focus of the decision phase. If no entity exceeds the "do nothing" threshold, the decision is "do nothing."

---

## Communication Philosophy

### Communication Is a Decision, Not a Default

The AI does not default to communicating. It defaults to silence. Every communication is a deliberate decision made by the cognitive loop, justified by the current state of the World Model.

### The Communication Decision

When the decision phase produces "ask," "suggest," or "notify," the AI must then determine:

1. **Should I communicate now?** — Based on interruption cost, user context, and urgency.
2. **Which interface?** — Desktop notification, voice, WhatsApp, email, terminal — chosen based on user's current context and past preferences.
3. **What should I say?** — The content of the communication, derived from the World Model entities that triggered it.
4. **How should I say it?** — Tone, length, detail level — learned from user's communication style and past responses.

### When to Stay Silent

The AI remains silent when:
- The information is not urgent and the user is in a focus state
- Confidence is too low to justify the interruption
- The user has previously ignored similar communications
- The situation is likely to resolve without intervention
- The cost of interrupting exceeds the value of the information

### Communication Types

| Type | Purpose | Example | Interruption Level |
|---|---|---|---|
| Ask | Get information only the user has | "Should I cancel the 3pm meeting?" | Medium — requires response |
| Suggest | Offer a recommendation | "Based on your calendar, tomorrow morning is free for deep work on AI-OS." | Low — can be ignored |
| Notify | Inform of a situation | "The project dependency is blocked." | Medium — time-sensitive |
| Explain | Provide context for an action | "I rescheduled the meeting because the participant is unavailable." | Low — informational |
| Encourage | Support user motivation | "You've made great progress on AI-OS this week." | Low — positive |
| Confirm | Verify before acting | "I can archive the old project files. Proceed?" | Medium — requires confirmation |
| Alert | Critical situation | "Your backup has failed 3 times in a row." | High — immediate |

### Learning Communication

The AI learns from every communication outcome:
- User responded positively → similar communications in similar contexts are reinforced
- User ignored → timing or content needs adjustment
- User responded negatively → communication type or context is deprioritised
- User asked for more detail → future communications include more context
- User asked to stop → communication type is suppressed for that topic

---

## Learning Model

### Every Cycle Is a Learning Opportunity

Learning is not a separate phase that runs periodically. It is an integral part of every cognitive cycle. The AI learns from everything:

| Source | What Is Learned | How It Updates the World Model |
|---|---|---|
| Accepted advice | What suggestions the user values | Increase importance of related entities; increase weight of predictive patterns |
| Rejected advice | What suggestions the user does not value | Decrease importance; decrease pattern weight; adjust timing |
| Ignored reminders | When not to communicate | Adjust interruption thresholds for this context |
| Completed actions | What actions are safe to automate | Increase confidence in action-outcome predictions |
| Cancelled actions | What actions the user does not want automated | Decrease confidence; add confirmation requirements |
| User corrections | Direct error feedback | Reduce confidence in incorrect entities; update properties |
| Unexpected outcomes | Prediction failure | Reduce confidence in prediction pattern; investigate cause |
| Environment changes | How the world behaves independently | Update prediction models; adjust importance |
| User state changes | User's energy, focus, availability | Learn patterns for optimal communication timing |
| Response time | How quickly user responds to different communication types | Optimise interruption thresholds per type |
| Repeated patterns | What happens regularly | Consolidate into higher-confidence predictions |

### What Learning Changes

Every learning event updates the World Model:

1. **Entity importance** — Recalculated based on new evidence
2. **Relationship weight** — Strengthened or weakened based on co-occurrence
3. **Confidence scores** — Increased with confirmation, decreased with contradiction
4. **Prediction patterns** — The AI's predictive models are adjusted
5. **Prioritisation weights** — What the AI considers important evolves
6. **Communication timing** — When to communicate and through which channel

### Learning Is Not Rule Creation

The AI never creates rigid rules. Learning is always probabilistic. A pattern observed 100 times is not a rule — it is a high-confidence prediction that remains open to contradiction. The AI can always be surprised.

This is essential for the system to remain adaptable. When the user's life changes — new job, new location, new priorities — the AI must be able to update its understanding completely. Rigid rules would prevent this.

### Forgetting

Forgetting is as important as remembering. The AI forgets when:
- Entity confidence drops below the forget threshold
- Entity importance drops below the archival threshold
- The user explicitly asks to forget
- A prediction pattern is consistently wrong
- A communication preference is consistently ignored

Forgetting is not deletion (unless the user requests it). It is archival — the entity is removed from active reasoning but preserved for potential recovery.

---

## Permission Model

### Continuous Permission Evaluation

The AI evaluates permissions at every stage of the cognitive loop, not just at execution time. Permissions are not a gate that opens once. They are continuously re-evaluated as context changes.

### Permission Checks

| Stage | Permission Checked | Decision |
|---|---|---|
| Observe | May I observe this source? | Some sources require explicit permission (email, calendar, location) |
| Interpret | May I understand this? | Some observations may be observed but not interpreted for privacy |
| Update World Model | May I store this entity? | Some entities may not be persisted |
| Reflect | Is this entity within bounds? | Some entities are off-limits for reasoning |
| Predict | May I make predictions about this? | Some domains may be excluded from prediction |
| Notify | May I interrupt for this? | User configures interruption permissions per category |
| Execute | May I perform this action? | Most restricted — requires explicit grant |
| Learn | May I learn from this outcome? | Some outcomes may be observed but not learned from |

### Permission Granularity

Permissions are per-entity-type, per-entity, and per-relationship:

```
Source-level:   User can observe my email? Y/N
Entity-level:   User's location may be stored? Y/N (N = observe but don't persist)
Domain-level:   User's health data may be reasoned about? Y/N
Action-level:   AI may reschedule my meetings? Y/N
Communication:  AI may notify me about calendar conflicts? Y/N
Learning:       AI may learn from my calendar patterns? Y/N
```

Default is deny. Every permission must be explicitly granted. The user can revoke any permission at any time.

### Permission Evolution

The AI may request permission expansion:
- "I notice I can't store calendar events. May I do that to help with scheduling?"
- "I'd like to learn from your sleep patterns to give better health suggestions. May I?"

The user can grant, deny, or grant with limitations. The AI respects the decision and learns not to ask again for denied permissions (unless circumstances change significantly).

---

## Integration with ADR-0006 World Model

### The Two ADRs Together

ADR-0006 answers "what do I know?" — the World Model.

ADR-0007 answers "how do I continuously think?" — the Cognitive Loop.

They form a complete picture:

```
World Model (ADR-0006)       Cognitive Loop (ADR-0007)
─────────────────────        ────────────────────────
Static knowledge             Continuous process
Entities & relationships     Observation, reflection, decision
What is stored               How it is used
The body of knowledge        The mind that thinks about it
```

### How the Loop Uses the World Model

| Loop Phase | World Model Operation |
|---|---|
| Observe | Perception layer extracts entities and relationships from raw input |
| Interpret | Entities are resolved against existing World Model |
| Update World Model | New entities and relationships are inserted; existing ones updated |
| Reflect | Graph traversal from changed entities; subgraph analysis |
| Predict | Pattern matching against historical entity states |
| Prioritise | Entity importance, urgency, and context from World Model properties |
| Decide | Decision criteria evaluated against World Model state |
| Communicate | Context subgraph provides the content for communication |
| Execute | Action entities created in World Model; Execution layer consumes them |
| Observe Result | Outcome observed; entities and relationships updated |
| Learn | Entity importance, relationship weight, confidence recalculated |

### Shared Data Model

Both ADRs share the same data model:
- **Entity** — nodes in the graph (people, projects, tasks, conversations, predictions, etc.)
- **Relationship** — edges in the graph (owns, depends_on, produces, predicts, etc.)
- **Importance** — computed from graph centrality and usage (0.0 - 1.0)
- **Confidence** — certainty of existence or accuracy (0.0 - 1.0)
- **Lifecycle** — Active, Archived, Forgotten

The Cognitive Loop extends the World Model with these additional entity types:
- **Prediction** — predicted future state with horizon, confidence, and evidence
- **Hypothesis** — possible explanation for an observed anomaly
- **Decision** — record of a decision made, including the reasoning
- **LearningEvent** — record of what was learned and from what evidence

These are not new data types — they are entity types like any other, with entity_type strings "prediction", "hypothesis", "decision", and "learning_event".

---

## Relationship to Existing Architecture

### No New Layers, No New Components

The Cognitive Loop operates entirely through the existing architecture. It does not introduce:
- New architecture layers
- New platform services
- New runtime components
- New daemons or background processes
- New crate categories
- New modules with hard boundaries

### How Existing Layers Participate

| Layer | Role in the Cognitive Loop |
|---|---|
| **OSAL** | Provides raw system observations (file changes, process events, network state) and raw system actions (file operations, process control). No change. |
| **Core** | EventBus carries cognitive events (world model updates, decisions, learning events) between components. Lifecycle management unchanged. No change. |
| **Runtime** | Task scheduling for background cognitive cycles, permission checking for every decision, session context for observation scoping. No change. |
| **Memory** | World Model store — persists entities, relationships, and their properties. Provides graph traversal for reflection and context generation. No change to infrastructure; purpose changes as described in ADR-0006. |
| **Perception** | Entity and relationship extraction from raw observations. No change to infrastructure; purpose changes as described in ADR-0006. |
| **Brain** | Runs the cognitive loop. The existing `CognitiveLoop::tick()` becomes the heartbeat of continuous cognition rather than a work-driven cycle. The loop's philosophy changes; its implementation structure (reflection, prediction, decision, learning) maps directly to existing brain crate subdivisions. |
| **Execution** | Consumes action entities from the World Model and executes them. No change. |
| **Intelligence** | Provides model-based reasoning for deep reflection cycles, entity extraction for perception, and natural language generation for communication. No change. |
| **Desktop Agent** | One of many observation sources (desktop events, user interactions) and communication channels (notifications, suggestions). No change. |

### What Changes

Only the behaviour of the Brain layer changes:
- The cognitive loop runs continuously, not on demand
- The default decision is "do nothing" rather than "find work"
- Reflection is triggered by any World Model change and by time passage, not only by goals
- Prediction is integrated into every cycle
- Communication decisions emerge from reasoning, not from schedules or triggers
- Learning happens on every cycle, not just after goal completion

The implementation path: the existing `brain-coordinator` crate's `CognitiveLoop::tick()` method gains a heartbeat timer (in addition to event-driven invocation). The tick logic reframes from "process next goal" to "process current World Model state". The existing sub-crates (brain-reasoner, brain-reflection, brain-learning, etc.) provide the same infrastructure but are composed differently by the coordinator.

---

## Failure Model

### When the AI Is Uncertain

The Cognitive Loop has explicit responses to uncertainty:

| Situation | Response |
|---|---|
| Entity confidence < threshold(low) | Do not use entity in reasoning. Consider observing again or asking. |
| Prediction confidence < threshold(low) | Do not act on prediction. Do not communicate about it. Consider whether to investigate. |
| Multiple contradictory interpretations | Prefer the interpretation with highest cumulative confidence. Flag contradiction for learning. If stakes are high, ask the user. |
| World Model state is inconsistent | Flag for learning. If inconsistency affects important entities, escalate to the user with an explanation. |
| User behaviour contradicts World Model | Reduce confidence in affected entities. The user is the ground truth. Update understanding to match. |
| Unexpected observation with no explanation | Create hypothesis entity. Observe. Wait for more data. If pattern persists, learn. |

### General Principles

- **When uncertainty is high:** Prefer asking.
- **When confidence is low:** Prefer waiting.
- **When consequences are significant:** Require confirmation.
- **When stakes are low and confidence is high:** Act.
- **When in doubt:** Do nothing. The cost of unnecessary action almost always exceeds the cost of delay.

---

## Success Metrics

The Cognitive Loop succeeds when the AI:

| Metric | What It Measures |
|---|---|
| Interruption rate | How often does the AI initiate communication? Should decrease over time as it learns what matters. |
| Helpfulness score | When the AI does communicate, does the user find it useful? Should increase over time. |
| Prediction accuracy | How often do predictions match reality? Should improve over time. |
| Understanding depth | Does the World Model accurately reflect the user's world? Measured by user corrections decreasing over time. |
| Learning rate | How quickly does the AI adapt to changes in the user's life? New job, new location, new priorities — how many cycles to update? |
| Instruction frequency | How often does the user need to explicitly tell the AI what to do? Should decrease over time. |
| Silence appropriateness | Does the AI stay silent when silence is appropriate and communicate when communication is needed? The hardest metric — evaluated by user satisfaction. |
| Companion feeling | Does the AI feel like a trusted companion rather than software? Subjective, measured by user feedback and engagement patterns. |

---

## Consequences

### Positive

- **True continuous intelligence.** The AI never stops thinking. It is always aware, always learning, always ready. This is the fundamental difference between a chatbot and a persistent intelligence.

- **Silence as a feature, not a bug.** The AI does not need to fill every silence with communication. It understands that silence is the default and communication must be justified.

- **Learning without instruction.** Because the cognitive loop always runs, the AI learns from every interaction, every observation, every outcome — without needing explicit training or feedback.

- **Adaptive prioritisation.** The AI continuously reprioritises based on the current state of the user's world. Nothing is fixed. Everything can be re-evaluated at any time.

- **Emergent communication.** Communication emerges naturally from reasoning about the World Model. It is not scheduled, not templated, not rule-based. Every communication is a genuine decision driven by understanding.

- **Single coherent philosophy.** ADR-0006 (World Model) and ADR-0007 (Cognitive Loop) together form a complete philosophical foundation. The AI has both knowledge (the model) and cognition (the loop).

- **No architectural disruption.** The entire cognitive loop operates through existing layers. No new components, no new services, no new runtime requirements. The infrastructure is unchanged.

### Negative

- **Philosophical shift requires cultural adoption.** The team must internalise "continuous cognition" as the mode of thinking. Old habits (prompt-response, goal-driven, workflow-based) will resurface. Discipline is required.

- **The "do nothing" decision is hard to test.** How do you verify that the AI correctly decided to stay silent? This requires new testing approaches — simulation, scenario evaluation, outcome metrics rather than output assertions.

- **Learning is slow to evaluate.** Improvement over time is hard to measure in short development cycles. The team must commit to long-term evaluation.

- **Risk of overthinking.** Without careful cognitive budget management, the AI could spend significant resources on shallow cycles. The depth-of-reflection model must be implemented thoughtfully.

- **User perception.** Users accustomed to chatbots may find the AI's silence unsettling at first. They may interpret silence as the system not working. Onboarding and expectation management are needed.

- **The line between cognition and execution is subtle.** The Cognitive Loop decides. Execution performs. But the loop also observes, interprets, and reflects — which are themselves actions. Clear implementation boundaries are needed in practice even though the philosophy treats them as one continuous process.

---

## Compliance

1. All future Brain Platform implementation must be guided by the Continuous Cognitive Loop philosophy. The loop is the behaviour; the Brain platform crates are the infrastructure that implements it.

2. The Brain's cognitive loop must run continuously (event-driven + heartbeat), not only on demand. A system that only thinks when prompted is not compliant with this ADR.

3. Every cognitive cycle must produce exactly one explicit decision. "Do nothing" is a valid decision that must be consciously produced, not an implicit default.

4. Communication must be a decision outcome, not a default behaviour. No component outside the cognitive loop may initiate user-facing communication.

5. Silence must be the default. The burden of justification is on communication, not on silence.

6. Learning must happen on every cycle, not periodically. Every observation of an outcome is a learning opportunity.

7. Predictions must have explicit confidence scores and time horizons. Predictions without confidence are not predictions — they are guesses.

8. No cognitive loop component may create rigid rules. All learning must be probabilistic and revisable.

9. The existing architecture layers (Core, Runtime, Memory, Brain, Perception, Execution, OSAL, Intelligence) must not be modified to accommodate the Cognitive Loop. The loop must operate within their existing interfaces.

10. The Cognitive Loop must be interface-independent. It must work the same whether the user interacts through desktop, voice, WhatsApp, email, or any future interface.

---

## Notes

- This ADR completes the philosophical foundation begun by ADR-0006. Together they define what the AI knows (World Model) and how it thinks (Cognitive Loop).
- The implementation phases in ADR-0006's roadmap (Phases 1-7) are compatible with this ADR. Phase 4 (Reasoning Over World Model) is where the Cognitive Loop behaviour is implemented.
- The existing `brain-coordinator` crate with its `CognitiveLoop::tick()` method is the natural implementation home for this ADR. The method's logic changes from "process next goal" to "process current World Model state."
- The reflection depth model (shallow/medium/deep/profound) provides a natural mapping to cognitive budget allocation in the existing `brain-core`'s `CognitiveBudget` type.
- This ADR's decision model replaces the previous goal-driven decision approach. Goals are now entities in the World Model rather than the driver of the cognitive loop.
- The user's relationship with the AI changes: from "ask and receive" to "live with a silent intelligence that occasionally speaks." This is intentional and central to the philosophy.

## References

- [ADR-0006: World Model Architecture](./0006-world-model.md) — The World Model that the Cognitive Loop reasons over.
- [ADR-0001: Project Vision](./0001-project-vision.md) — Establishes the project's long-term vision for intelligence.
- [ADR-0002: Clean Architecture](./0002-clean-architecture.md) — The layered architecture that the Cognitive Loop operates within.
- [ADR-0004: Event-Driven Design](./0004-event-driven.md) — The EventBus that carries cognitive events between layers.
- [World Model Architecture](../architecture/world-model.md) — The detailed architecture of the World Model data model, store, and operations.
- [Brain Platform Architecture](../architecture/brain.md) — The Brain infrastructure that implements the cognitive loop.
- [Memory Platform Architecture](../architecture/memory.md) — Physical storage for the World Model.
- [Perception Platform Architecture](../architecture/perception.md) — Entity extraction from raw observations.
- [Execution Platform Architecture](../architecture/execution.md) — Action execution.
- [RFC-0003: Brain Platform](../rfc/RFC-0003-brain-platform.md) — The Brain platform RFC (to be updated to reflect this ADR).
- [Cognitive Loop Detail](../architecture/cognitive-loop.md) — Technical details of the Brain's cognitive loop implementation.
