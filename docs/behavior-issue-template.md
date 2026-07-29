# Behavior Issue Template

Report a behavioral issue with AI-OS using this template. A complete issue helps the team diagnose and fix problems quickly.

---

## Template

### Situation

**What were you doing when the issue occurred?**

Describe the context: what commands you ran, what you asked the companion, what system state you were in. Include timestamps if relevant.

Example:
> I asked the companion "What did I work on this morning?" at 10:15 AM. The companion had been running for 2 hours with the observation loop active.

### Expected Behavior

**What did you expect the companion to do?**

Be specific about what the companion should have said, done, or decided.

Example:
> I expected the companion to recall that I ran `cargo test` on the brain-coordinator project this morning, since that information was observed and persisted.

### Actual Behavior

**What did the companion actually do?**

Include exact outputs, transcripts, or screenshots where possible.

Example:
> The companion responded: "I don't recall any morning activity." No entities related to `cargo test` or `brain-coordinator` were found in the World Model.

### Observed Reasoning

**What did the companion's decision chain look like?**

If the dashboard or debug mode is enabled, include the decision reasoning here. Otherwise, describe what you think the companion was processing.

Example:
> The companion's decision log shows: Observe → no relevant observations in the last cycle → Reflect → confidence 0.1 → Communicate → "I don't recall..."

### Responsible Subsystem

**Which subsystem is likely at fault?**

Choose from:
- `observation` — the companion missed or failed to observe
- `memory` — the observation was not persisted correctly
- `reflection` — the companion failed to find the observation during reflection
- `decision` — the companion made the wrong decision despite having the data
- `attention` — attention signals were missed
- `persistence` — data was lost on shutdown/restart
- `notification` — notification was not delivered or was spammy
- `privacy` — privacy settings prevented data storage or access

### Root Cause

**What is your best hypothesis for the root cause?**

Example hypotheses:
> The observation for `cargo test` occurred but the reflection loop had not yet run to process it when the query was made. The companion queries the World Model at query time, but the observation from this morning may not have been persisted yet due to a crash or interrupted shutdown.

### Minimal Fix

**If you know the fix, describe it.**

Example:
> Ensure the observation is persisted before the next reflection cycle runs. Add a `store_observation` call at the end of each observation cycle before the next reflect cycle starts.

### Regression Scenario

**How can this issue be reproduced?**

List steps that trigger the issue. This should be a minimal, repeatable set of steps.

Example:
1. Start the companion fresh
2. Open a terminal and run `cargo test -p brain-coordinator`
3. Wait for the observation cycle to process
4. Stop the companion (`Ctrl+C`)
5. Restart the companion
6. Ask "What did I work on?"
7. Observe that the companion does not recall the test run

### Additional Context

**Any other relevant information:**

- Companion version (from `/api/status`)
- World Model size (from `/api/entities` count)
- Any recently changed settings
- Whether encryption is enabled
- Whether the issue is reproducible or intermittent
