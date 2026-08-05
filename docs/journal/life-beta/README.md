# Behavior Journal: LIFE Beta

This journal captures Phase 4 — validating LIFE as a daily companion through
real-world use. Entries follow the real-time improvement loop: observe, trace
root cause, fix minimally, verify.

The metrics evaluated are cognitive, not executional:

- **Memory continuity** — does the companion remember context across sessions?
- **Long-term understanding** — does the companion build a durable model of the user?
- **Attention quality** — does the companion focus on what matters, not noise?
- **Decision quality** — are the chosen actions appropriate to the situation?
- **Capability selection** — does the companion pick the right tool for the job?
- **Communication quality** — is every message natural, never system-speak?
- **User trust** — does interaction feel reliable and safe?
- **Recovery after mistakes** — how does the companion handle errors?

## Entry Template

Copy this into a new numbered file (`NNN-<short-title>.md`) for each session.

```markdown
# Behavior Journal: LIFE Beta

---

## Entry NNN — <short title>

**Date**: <YYYY-MM-DD>
**Session start**: <UTC time>
**Capabilities available**: <comma-separated list>
**Policy gate**: <initialized / not initialized>

### Situation

<what the user expected or tried to do in this session>

### Observation

<what LIFE actually did, verbatim quotes where relevant>

### Understanding

<root-cause trace: which cognitive component is responsible, file:line>

### Decision

<the fix LIFE applied or should apply>

### Outcome

<whether the situation was resolved, and how; or "open — see next entry">
```
