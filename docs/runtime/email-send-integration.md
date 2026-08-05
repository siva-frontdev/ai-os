# First Integration — `email.send`

**Status:** Implemented (mock MCP email plugin; real OpenClaw extension pending)
**Date:** 2026-08-05
**Scope:** The first end-to-end integration of the Runtime boundary: a user asks AI-OS (over Telegram) to send an email, and AI-OS executes it through the Runtime API. This is the minimal proof that "AI-OS owns cognition, the runtime owns execution."

---

## 1. Key Finding First

OpenClaw ships **no email channel** (`extensions/` has no email/gmail/smtp/imap; the channel config schema has no email key). Two ways to close the gap:

| Option | Description | Pros | Cons | Recommendation |
|---|---|---|---|---|
| **A. Email channel extension** | Build a small OpenClaw channel extension (`email`) following the `ChannelPlugin` contract (SMTP outbound + IMAP inbound), installed into the Gateway sidecar via OpenClaw's plugin system as a separate package. | Keeps the "runtime provides capabilities" story clean; email is just another connector; uses OpenClaw's extension architecture. | Requires authoring a TypeScript extension; the cloned OpenClaw repo stays untouched (the extension is a separate package). | **Recommended for v1.** |
| **B. Rust SMTP connector in adapter** | Implement `email.send` directly in `openclaw-runtime` (e.g. `lettres` crate), bypassing OpenClaw for email. | No TypeScript; fully Rust. | Blurs the boundary (AI-OS→OpenClaw→capability); email would not work for other runtimes. | Fallback if A is delayed. |

The design below is transport-agnostic at the Runtime API level: the brain does not care which option delivers the email.

---

## 2. Capability Declaration

```json
{
  "id": "email.send",
  "name": "Send email",
  "description": "Send an email to one or more recipients. Use when the user asks to email someone.",
  "input_schema": {
    "type": "object",
    "required": ["to", "subject", "body"],
    "properties": {
      "to":        { "type": "string" },
      "subject":   { "type": "string" },
      "body":      { "type": "string" },
      "cc":        { "type": "array", "items": { "type": "string" }, "default": [] }
    }
  },
  "output_schema": {
    "type": "object",
    "properties": {
      "message_id": { "type": "string" },
      "sent_at_ms": { "type": "integer" }
    }
  },
  "side_effects": ["SendsMessage"]
}
```

This capability is merged into the brain's `CapabilityRegistry` at runtime init (`runtime.initialize()` → `capabilities()` → merge). The Planner can then legally plan an `email.send` action — **no planner code change**.

---

## 3. End-to-End Flow

```
┌─────────┐  1. "email bob@example.com: report is ready"
│ Telegram│──────────────────────────────►  OpenClaw Gateway (telegram channel extension)
└─────────┘                                       │ 2. inbound event → classify → envelope
                                                  ▼
                                 Runtime API  observe()/subscribe()
                                                  │ 3. Observation { source: telegram.receive,
                                                  │    payload: { text, chat_id, sender } }
                                                  ▼
┌──────────────────────────────────────────────────────────────────────────┐
│                        AI-OS Cognitive Loop                               │
│  ┌─────────────────┐ ┌──────────────────┐ ┌────────────────────────────┐ │
│  │ World           │ │ Memory Evaluator │ │ Evolution (persist:         │ │
│  │ Understanding   │ │ (store? no)      │ │ "user emailed from Telegram")│ │
│  └────────┬────────┘ └─────────┬────────┘ └────────────┬───────────────┘ │
│           └─────────┬──────────┴───────────────────────┘                 │
│                     ▼                                                    │
│   Planner — capabilities: respond, search_memory, email.send, …          │
│   plan: [ { type: "email.send", topics: [], reason: "user asked" } ]     │
│                     │                                                    │
│                     ▼                                                    │
│   Decision: Execute → RuntimeDispatcher.dispatch(email.send, input)      │
└──────────────────────────────────────────────────────────────────────────┘
                     │
                     ▼
        ┌────────────────────────── openclaw-runtime ──────────────────────────┐
        │ validate input vs email.send input_schema                           │
        │ capability_map: "email.send" → email channel tool "email.send"      │
        │ action_map: {to, subject, body} → tool args                         │
        │ gateway_client.call_tool("email.send", args)   (MCP tools/call)     │
        └────────────────────────────────┬────────────────────────────────────┘
                                         │
                                         ▼
              OpenClaw Gateway → email channel extension → SMTP → recipient
                                         │
                                         ▼
              Result { message_id, sent_at_ms } ──► result_map ──► ActionResult{ Succeeded }
                                         │
                                         ▼
                        Cognitive Loop ──► Decision: Communicate
                                         │
                                         ▼
        Runtime API: telegram.post_message ──► Telegram channel outbound
                                         │
                                         ▼
        "Sent. Bob@example.com should see it shortly."
```

### Steps (numbered)

| # | Stage | Owner | Detail |
|---|---|---|---|
| 1 | Inbound message | Telegram channel ext | User sends the request over Telegram. |
| 2 | Inbound ingestion | Gateway / channel runtime | Event classified `user_request`, enveloped, bound to session. |
| 3 | Observation | Runtime API | `OpenClawRuntime` maps envelope → `Observation`; pushed via `subscribe()`. |
| 4 | Interpretation | World Understanding | Entities: `Bob@example.com` (email), subject/body intent; no cognitive keywords. |
| 5 | Memory | MemoryEvaluator + Evolution | Optionally persist "user uses Telegram for email" (importance-based, LLM decided). |
| 6 | Planning | Planner | Plans `email.send` because the capability is registered. |
| 7 | Decision | Cognitive Loop | Execute the plan via the RuntimeDispatcher. |
| 8 | Action | Runtime API | `Action { capability: "email.send", input: {...} }`. |
| 9 | Translation | openclaw-runtime | Schema validate → capability_map → action_map → tool call. |
| 10 | Execution | OpenClaw email connector | SMTP send; returns message id. |
| 11 | Result | Runtime API | `ActionResult { status: Succeeded, output: { message_id, sent_at_ms } }`. |
| 12 | Response | Cognitive Loop → Communication Logic | Decide to reply; emit Telegram reply. |
| 13 | Delivery | Telegram channel ext | Reply text delivered via outbound `sendText`. |

**No OpenClaw planner, memory, or reasoning executes at any step.** Steps 4–7 are 100% AI-OS; steps 1–3 and 10–13 are 100% runtime.

---

## 4. What This Proves

1. **AI-OS owns cognition** — the loop interprets, evaluates, plans, decides, and composes the reply.
2. **Runtime owns execution** — sending the email is a connector call; AI-OS never builds SMTP packets.
3. **OpenClaw is one implementation** — if the email capability moved to Home Assistant, only the wiring changes.
4. **No duplicate brain** — OpenClaw's `runEmbeddedAgent`, prompt builder, memory, and auto-reply decisioning are never invoked.
5. **Replaceable** — `openclaw-runtime` ↔ `native-runtime` swap is a wiring change only.

---

## 5. Verification Plan

| Check | How |
|---|---|
| Capability merged | Planner prompt lists `email.send` after init; `runtime.capabilities()` contains it. |
| Email sent | Integration test against a mock SMTP server (e.g. `smtp4dev` / MailHog); assert `ActionResult::Succeeded` with `message_id`. |
| Reply delivered | Telegram test chat receives "Sent…" message. |
| No OpenClaw cognition | Assert OpenClaw logs contain zero `runEmbeddedAgent` / agent-loop invocations during the flow. |
| Schema validation | Action with missing `to` → `Failed { code: "invalid_input" }` before any SMTP call. |
| Unknown capability | Planner cannot emit a capability not in the registry; dispatcher returns `capability_not_found`. |

---

## 6. Out of Scope (later phases)

- Email **receive** (IMAP ingestion → AI-OS) — requires the email channel extension's inbound side.
- Attachments, threading, signatures.
- Other capabilities (web search, browser, Home Assistant, ROS).
