# OpenClaw → AI-OS Mapping

**Status:** Draft
**Date:** 2026-08-05
**Purpose:** Exhaustive mapping of every major OpenClaw subsystem to its AI-OS counterpart, with an explicit decision. This is the authoritative answer to "what does AI-OS own, and what does the runtime own."

**Legend:** **Keep** = reuse as runtime behind the Runtime API · **Remove** = must not run in the integrated flow; AI-OS supersedes it · **Ignore** = not used · **Gap** = missing, must be provided.

---

## 1. Module Mapping Table

| OpenClaw Module | AI-OS Layer | Decision | Notes |
|---|---|---|---|
| Gateway (control plane) | Execution Runtime (layer 7) | **Keep** | Long-running host process for channels + tools. AI-OS drives it via RPC/MCP. Bypass its system-agent/prompt builder. |
| Gateway protocol / gateway-client | Runtime API adapter | **Keep** | Typed WS RPC client. Preferred boundary is MCP over stdio/HTTP (§2.7 analysis). |
| Worker (turn executor) | Execution Runtime | **Keep** (sandboxed turn) / **Remove** (its reasoning) | If AI-OS ever delegates a turn; primary flow calls tools directly. |
| CLI | — | **Ignore** | Manual ops only. |
| Daemon (systemd/launchd) | services/ (AI-OS) | **Ignore** | AI-OS owns process supervision. |
| Config system | configs/ (AI-OS) | **Ignore** | Adapter holds OpenClaw config in isolation. |
| State (SQLite) | AI-OS state | **Ignore** | AI-OS owns durable state. |
| Status / UI / apps | — | **Ignore** | Presentation; out of initial scope. |
| Channels abstraction (`src/channels/`) | Runtime API / Cognitive Loop input | **Keep** | Uniform inbound/outbound transport contract. |
| Channel extensions (telegram, whatsapp, slack, signal, discord, imessage, …) | Runtime API connectors | **Keep** | Communication runtime ("hands and legs"). |
| **Email channel** | Runtime API connector | **Gap** | **Does not exist in OpenClaw.** Provide via AI-OS Runtime-API connector or a new OpenClaw channel extension. |
| Tool contract (`AgentTool`, `src/tools/`) | Capability Registry → Runtime API | **Keep** | Tools are callables; invocable without the agent loop. |
| MCP server/client | Runtime API boundary | **Keep** | Generic `tools/list`/`tools/call` — the recommended AI-OS↔OpenClaw seam. |
| Plugin registry (`OpenClawPluginApi`) | Runtime API connector loading | **Keep** | registerTool/registerChannel surface. |
| Plugin SDK / package contract | runtime-api packaging | **Keep** | Extension authoring contract. |
| Agent turn loop (`runEmbeddedAgent`, `agent-loop.ts`) | AI-OS Cognitive Loop | **Remove** | This is OpenClaw's reason/act loop. Never runs. |
| Stateful `Agent` wrapper | AI-OS Cognitive Loop | **Remove** | Superseded by AI-OS loop. |
| System prompt / bootstrap / prompt surface | AI-OS Planner + World Understanding | **Remove** | Prompt building is cognition. |
| Compaction / summarization | AI-OS Memory + Reflection | **Remove** | Memory compression is cognition. |
| Memory search / embedding / flush | AI-OS World Model + MemoryEvaluator | **Remove** | Remembering is cognition. (Storage backend may be reused inertly.) |
| Memory write-provenance | AI-OS Memory | **Keep as pattern** | Runtime-safe pattern for auditable writes. |
| Sessions (SQLite transcript store) | AI-OS conversation history | **Keep** (storage) | Channel-side record; AI-OS keeps its own loop history. |
| Session lifecycle/admission | AI-OS Runtime + Cognitive Loop | **Ignore** | Who-may-speak is decided by AI-OS. |
| Cron / Tasks | AI-OS Scheduler (runtime crate) | **Keep** (scheduler) / **Remove** (its turns) | Scheduled work calls AI-OS, not `runEmbeddedAgent`. |
| Auto-reply decisioning | AI-OS Cognitive Loop | **Remove** | Reply decision belongs to AI-OS. Keep inbound dispatch/debounce plumbing. |
| Skills prompt-injection | AI-OS Planner | **Remove** | Prompt content is cognition. |
| Skills install/discovery/dispatch | runtime-api capability packaging | **Keep (optional)** | Reusable machinery, re-purposed. |
| LLM transports (`packages/ai`, `llm-core`) | AI-OS intelligence stack | **Keep (optional adapter)** | AI-OS has its own coordinator; not required. |
| Model provider extensions | AI-OS intelligence stack | **Ignore** | AI-OS uses its own intelligence stack. |
| Tools/capability extensions (browser, document-extract, web-search, …) | Runtime API connectors | **Keep** | Capabilities on demand. |
| Memory extensions (active-memory, lancedb, wiki, vault) | AI-OS Memory | **Remove** | Cognition. |
| Ops extensions (otel, prometheus, admin-http-rpc, webhooks) | AI-OS observability | **Keep** | Ops. |
| Commitments extraction | AI-OS Reflection/Memory | **Remove** | Cognition. |
| Meeting-bot / realtime voice | — | **Ignore** (initial) | Optional later surface. |
| Subagent registry / orchestration | AI-OS Planner | **Remove** | Cognition. |
| Tool-loop detection / repair | Execution Runtime safety | **Keep** | Runtime safety, not cognition. |

---

## 2. Decision Summary

| Decision | Count | Subsystems |
|---|---|---|
| **Keep** | 14 | Gateway, protocol/client, channels abstraction + extensions, tool contract, MCP, plugin registry/SDK, sessions store, cron/tasks scheduler, tool-loop safety, ops extensions, LLM transports (optional), tool capabilities |
| **Remove** | 10 | Agent loop, `Agent` wrapper, prompt assembly, compaction, memory search/embedding, auto-reply decisioning, skills injection, commitments, subagent orchestration, memory extensions |
| **Ignore** | 8 | CLI, daemon, config, state, status, UI/apps, model providers, session lifecycle |
| **Gap** | 1 | Email channel (must be provided) |

---

## 3. Ownership Statement

> **AI-OS owns cognition.** Understand, Remember, Reason, Plan, Reflect, Learn, Decide.
> **The runtime owns execution.** Expose capabilities, execute actions, communicate with external systems, provide observations, manage connectors/plugins.
> **OpenClaw is one runtime implementation.** Its planner, memory, prompts, reasoning loop, and conversations are inert in the integrated flow.

```
AI-OS (brain)                OpenClaw (hands & legs)
─────────────────            ─────────────────────────
World Model                  (inert — no OpenClaw memory)
World Understanding          (inert — no OpenClaw prompt building)
Cognitive Loop               (inert — runEmbeddedAgent never called)
Planner                      (inert — no OpenClaw planning)
Evolution                    (inert — no OpenClaw compaction)
Reflection / Attention       (inert — no OpenClaw subagents)
Memory                       (inert — no OpenClaw memory search)
Decision                     (inert — no OpenClaw auto-reply decisioning)
Communication Logic          (inert — no OpenClaw conversation ownership)
                              │
                              ▼
Runtime API ──► openclaw-runtime adapter ──► Gateway RPC / MCP
                                                ├── channels (telegram, slack, …)
                                                ├── tools (browser, web, …)
                                                └── plugins/connectors
```
