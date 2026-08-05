# OpenClaw Runtime Analysis

**Status:** Draft
**Phase:** Phase 8 — Execution Platform
**Date:** 2026-08-05
**Audience:** AI-OS Architecture Review
**Scope:** Analysis of the OpenClaw codebase as an execution runtime for AI-OS. OpenClaw is NOT an AI agent in this architecture. It is the hands and legs — capabilities, connectors, communication, and execution. AI-OS is the brain.

## 0. Executive Summary

OpenClaw (`openclaw/openclaw`, analyzed at commit `80da6166`, checked out read-only at `~/openclaw`) is a large TypeScript/pnpm workspace (~555 MB, 161 bundled extensions) that ships a personal AI assistant: a Gateway control plane, 20+ messaging channels, a tool/plugin registry, an MCP server and client, and an embedded agent runtime.

For AI-OS, the analysis yields a clean split:

- **KEEP as execution runtime:** Gateway control plane, `gateway-client`/`gateway-protocol` (typed WS RPC), channel abstractions + channel extensions (Telegram, WhatsApp, Slack, Signal, iMessage, …), the tool contract (`AgentTool`), MCP server/client bridging, the plugin registry (`OpenClawPluginApi`), sessions SQLite transcript store, cron/tasks scheduling, LLM transport adapters.
- **REMOVE FROM FLOW (brain concerns):** the agent turn loop (`runEmbeddedAgent`, `agent-loop.ts`), system-prompt / bootstrap / prompt assembly, compaction/summarization, memory search/embedding/flush, commitments extraction, auto-reply decisioning, skills prompt-injection.
- **IGNORE:** CLI, daemon, config system, state/status presentation, control UI, voice/canvas/nodes (unless later desired).
- **GAP:** **OpenClaw ships NO email channel.** There is no `email`/`gmail`/`smtp`/`imap` extension and no email key in the channel config schema. `email.send` (the recommended first integration capability) must therefore be provided either as a new OpenClaw channel extension or, cleaner, as a first-class connector on the AI-OS Runtime API side. This is detailed in `email-send-integration.md`.

The integration boundary is: **AI-OS (Rust) ↔ Runtime API trait ↔ `openclaw-runtime` adapter ↔ OpenClaw Gateway RPC / MCP.** AI-OS never imports OpenClaw types; OpenClaw never runs cognition.

---

## 1. Method

- Cloned `https://github.com/openclaw/openclaw` into `~/openclaw` (separate directory, untouched, shallow clone at `main`).
- Analyzed subsystems by directory: gateway, channels, tools, MCP, skills, plugins, agents, memory, sessions, cron/tasks, worker, packages.
- Decisions use the taxonomy: **KEEP** (reuse as-is or behind adapter), **REMOVE FROM FLOW** (exists, but must not run — AI-OS replaces the behavior), **REPLACE** (AI-OS provides a superseding implementation), **IGNORE** (not needed / host substitutes its own).

---

## 2. Module Analysis

### 2.1 Gateway — `src/gateway/`

| Field | Value |
|---|---|
| **Purpose** | Combined HTTP + WebSocket control plane. Authenticates clients, manages agent sessions, exposes ~60 RPC method families (`server-methods.ts`: `agent`, `chat`, `sessions`, `config`, `cron`, `devices`, `plugins`, `models`, `terminal`, `fs`, `exec-approvals`, `memory-search`, …). Serves the browser Control UI, MCP over HTTP, and OpenAI-compatible REST endpoints. |
| **Key files** | `server.ts`, `server.impl.ts`, `server-start.ts` (`startGatewayServer(port, opts)`), `server-ws-runtime.ts`, `server-methods.ts`, `auth.ts`, `node-registry.ts`, `agent-prompt.ts` |
| **Dependencies** | `config/`, `agents/agent-command.ts`, `auto-reply/`, `plugins/`, `sessions/`, `infra/`, `packages/gateway-protocol` |
| **Decision** | **KEEP** (control plane), with a bypass |
| **Notes** | Ships its own prompt builder (`agent-prompt.ts`) and a "system-agent" onboarding brain. AI-OS must drive the Gateway with explicit prompts via `chat.send` / direct RPC and never let the Gateway's own system-agent run unattended. The Gateway is the natural long-running process that hosts the channels and tool runtimes AI-OS needs. |

### 2.2 Gateway Protocol & Client — `packages/gateway-protocol/`, `packages/gateway-client/`

| Field | Value |
|---|---|
| **Purpose** | Typed WS RPC contract (`gateway-protocol`: typebox schemas, validators, error codes, protocol versioning) and the client library (`gateway-client`: `GatewayClient`, `GatewayProtocolClient`, reconnect, device auth). |
| **Key files** | `schema/frames.ts`, `client.ts`, `protocol-client.ts`, `session-subscriptions.ts`, `device-auth.ts` |
| **Dependencies** | typebox, ws |
| **Decision** | **KEEP** |
| **Notes** | This is the cleanest programmatic surface an external host can call. The Rust `openclaw-runtime` adapter can implement a thin WS client against this protocol, or — better — call OpenClaw's MCP server (see §2.7) over stdio/HTTP to avoid protocol drift. |

### 2.3 Worker — `src/worker/`

| Field | Value |
|---|---|
| **Purpose** | Subprocess turn executor. `runWorkerDescriptor()` connects to the Gateway over a local WS socket; `runWorkerEmbeddedTurn()` executes a single agent turn (builds the model, session, tools, transcript) under a tool-authority allowlist from the Gateway. |
| **Key files** | `worker.runtime.ts`, `embedded-agent.runtime.ts` |
| **Dependencies** | Gateway, `agents/`, `packages/ai`, `packages/llm-core` |
| **Decision** | **KEEP** as sandboxed turn executor; the reasoning loop it invokes is **REMOVE FROM FLOW** (see §2.8) |
| **Notes** | If AI-OS ever delegates a turn to OpenClaw, this is the sandboxed boundary. Primary integration does NOT use it: AI-OS calls tools directly. |

### 2.4 CLI — `src/cli/` | Daemon — `src/daemon/` | Config — `src/config/` | State — `src/state/`

| Module | Purpose | Decision | Notes |
|---|---|---|---|
| **CLI** | Commander-based `openclaw` CLI (`gateway`, `agent`, `send`, `onboard`, `doctor`). | **IGNORE** | AI-OS hosts call RPC/MCP, not a process CLI. Useful only for manual ops. |
| **Daemon** | systemd/launchd/schtasks service management for the Gateway. | **IGNORE** | AI-OS has its own service lifecycle (`services/`, systemd units). |
| **Config** | Large zod-schema config system (channels, agents, models, tools). | **IGNORE** | AI-OS supplies its own config; the adapter holds OpenClaw config in isolation. |
| **State** | SQLite persistence for OpenClaw's own state. | **IGNORE** | AI-OS owns durable state. |
| **Status** | Status-text formatting for CLI/UI. | **IGNORE** | Presentation only. |
| **Bootstrap** | Node child-process env (TLS CA certs). | **IGNORE** | Host substitutes its own. |

### 2.5 Channels & Connectors — `src/channels/` + `extensions/<channel>/`

| Field | Value |
|---|---|
| **Purpose** | Channel-agnostic messaging platform: plugin registry, durable inbound queue, outbound delivery, session binding, reply dispatch. 20+ transport implementations live under `extensions/` (telegram, whatsapp, slack, signal, discord, imessage, msteams, matrix, googlechat, zalo, feishu, twitch, line, …). |
| **Key files** | `src/channels/plugins/types.plugin.ts` (`ChannelPlugin`), `src/channels/message/types.ts`, `src/channels/plugins/types.adapters.ts` (`ChannelGatewayAdapter.startAccount/stopAccount`), `src/channels/plugins/outbound.types.ts` (`ChannelOutboundAdapter.sendText/sendMedia/sendPayload`), `extensions/telegram/`, `extensions/slack/` |
| **Dependencies** | per-channel SDKs: telegram → `grammy`, slack → `@slack/bolt`, whatsapp → `baileys`, signal → signal-cli bridge, imessage → osascript |
| **Decision** | **KEEP** |
| **Notes** | The channel contract is uniform: a channel implements `ChannelPlugin` with config adapter, `startAccount` (inbound listener: poll/webhook/socket) and outbound `sendText/sendMedia/sendPayload`. Channels are ~90% pure transport + session/thread binding. No reasoning, no planning. Inbound messages are classified (`user_request`/`room_event`), enveloped, and routed to the agent runtime — AI-OS intercepts this routing. **No email channel exists (§0).** |

### 2.6 Tools — `src/tools/`, `src/agents/agent-tools.ts`, `packages/llm-core`, `packages/agent-core`

| Field | Value |
|---|---|
| **Purpose** | The tool contract and registry. Core type `AgentTool = { name, description, parameters, execute(toolCallId, params, signal, onUpdate) }` returning `AgentToolResult`. `src/tools/types.ts` defines the `ToolDescriptor` metadata contract with owners (`core | plugin | channel | mcp`). |
| **Key files** | `src/tools/types.ts`, `src/agents/agent-tools.ts`, `packages/llm-core/src/types.ts:378` (`Tool`), `packages/agent-core/src/types.ts:497` (`AgentTool`) |
| **Dependencies** | none (pure contract) + tool implementations |
| **Decision** | **KEEP** |
| **Notes** | Tools are plain callables. The agent loop merely calls `tool.execute(...)`. **An external host can import and invoke tool implementations directly, bypassing the agent loop entirely** — this is the key property AI-OS relies on. Authorization is enforced by tool-policy/allowlist layers in `gateway/worker-environments/`. |

### 2.7 MCP — `src/mcp/`

| Field | Value |
|---|---|
| **Purpose** | OpenClaw is both an MCP **server** (exposes its tools outward via stdio/streamable-HTTP: `tools-stdio-server.ts`, `plugin-tools-serve.ts`) and an MCP **client** (consumes external MCP servers, materializing their tools as `AnyAgentTool`s). |
| **Key files** | `src/mcp/tools-stdio-server.ts`, `src/mcp/plugin-tools-serve.ts`, `src/mcp/plugin-tools-handlers.ts`, `src/agents/agent-bundle-mcp-runtime.ts`, `src/agents/agent-bundle-mcp-materialize.ts` |
| **Dependencies** | `@modelcontextprotocol/sdk` |
| **Decision** | **KEEP** |
| **Notes** | **Recommended AI-OS ↔ OpenClaw boundary.** AI-OS connects to the OpenClaw MCP server (`tools/list`, `tools/call`) over stdio or streamable-HTTP. No OpenClaw internals leak; the interface is MCP, a generic protocol; replacing OpenClaw with another MCP-capable runtime is trivial. |

### 2.8 Agent Runtime & Reasoning — `src/agents/` (loop), `packages/agent-core/`

| Field | Value |
|---|---|
| **Purpose** | OpenClaw's own reason/act loop: `runEmbeddedAgent()` → `agent-loop.ts` `runLoop()` (LLM call → tool use → execute tools → repeat), stateful `Agent` wrapper, thinking policy, tool-loop detection, compaction/summarization, `runCronCommandJob`, subagent registry. |
| **Key files** | `src/agents/embedded-agent.ts`, `src/agents/embedded-agent-runner/run-orchestrator.ts`, `packages/agent-core/src/agent-loop.ts`, `packages/agent-core/src/agent.ts`, `src/agents/compaction.ts`, `src/agents/thinking-runtime.ts`, `src/agents/subagent-registry.ts` |
| **Dependencies** | `packages/llm-core`, `packages/ai` (transports), `src/agents/agent-tools.ts`, sessions, memory |
| **Decision** | **REPLACE / REMOVE FROM FLOW** |
| **Notes** | This is OpenClaw's brain. AI-OS owns planning, reasoning, reflection, and decision-making. `runEmbeddedAgent`, the agent loop, compaction/summarization, subagent orchestration, and commitment extraction must **never** run in the integrated flow. They remain usable only for testing OpenClaw in isolation. The transport layer (`packages/ai`, `packages/llm-core` types) is fine to keep as an LLM adapter behind AI-OS's own coordinator. |

### 2.9 Prompt Assembly — `src/agents/system-prompt.ts` et al.

| Field | Value |
|---|---|
| **Purpose** | `buildAgentSystemPrompt()` — a ~1600-line assembler: tool summaries, memory section, context files, BOOTSTRAP mode, heartbeat prompt, model identity. |
| **Key files** | `src/agents/system-prompt.ts`, `src/agents/prompt-surface.ts`, `src/agents/bootstrap-prompt.ts` |
| **Decision** | **REMOVE FROM FLOW** |
| **Notes** | Prompt building is cognition. AI-OS's Planner, MemoryEvaluator, and response generation build all prompts. OpenClaw prompt assembly must not execute. |

### 2.10 Memory — `src/memory/`, `src/agents/memory-*.ts`, `src/plugins/memory-*.ts`

| Field | Value |
|---|---|
| **Purpose** | MEMORY.md/USER.md resolution, memory search (SQLite FTS+vector, hybrid ranking), memory write provenance, memory prompt-section preparation. |
| **Key files** | `src/memory/root-memory-files.ts`, `src/agents/memory-search.ts`, `src/plugins/memory-runtime.ts`, `src/agents/memory-write-provenance.ts`, `extensions/memory-*`, `extensions/active-memory` |
| **Decision** | **REMOVE FROM FLOW** (storage may be kept only as inert backend) |
| **Notes** | Remembering is cognition owned by AI-OS (World Model, MemoryEvaluator, Evolution). OpenClaw's memory search/embedding/flush and MEMORY.md injection must not run. The write-provenance pattern (tracking which writes came from which turn, for rollback) is a good pattern AI-OS may adopt — it is a runtime concern, not cognition. |

### 2.11 Sessions — `src/sessions/`, `src/agents/sessions/`, `src/transcripts/`

| Field | Value |
|---|---|
| **Purpose** | SQLite-backed session tree, transcript persistence (SQLite rows + JSONL artifacts), session lifecycle/admission. |
| **Key files** | `src/agents/sessions/session-manager*.ts`, `src/sessions/`, `src/transcripts/store-sqlite.ts` |
| **Decision** | **KEEP** (durable transcript store) / **IGNORE** (lifecycle admission) |
| **Notes** | Storage, not cognition. AI-OS keeps its own conversation history in the Cognitive Loop; OpenClaw's transcript store can remain as the channel-side record. The session lifecycle/admission logic that decides "who may start a turn" is superseded by AI-OS. |

### 2.12 Cron / Tasks / Auto-reply — `src/cron/`, `src/tasks/`, `src/auto-reply/`

| Module | Purpose | Decision | Notes |
|---|---|---|---|
| **Cron** | Scheduled jobs; runs scheduled turns. | **KEEP** (scheduler) / REMOVE FROM FLOW (its turns) | Scheduling is runtime; the scheduled work must call AI-OS, not `runEmbeddedAgent`. |
| **Tasks** | Durable task/flow records, detached runs, delivery. | **KEEP** | Maps to AI-OS Runtime/Scheduler concepts. |
| **Auto-reply** | Channel-driven trigger that starts OpenClaw's own reasoning. | **REMOVE FROM FLOW** (decisioning); keep inbound dispatch/debounce plumbing | The decision to reply belongs to AI-OS. |

### 2.13 Skills — `src/skills/`

| Field | Value |
|---|---|
| **Purpose** | Markdown SKILL.md instruction packages: discovery, install (clawhub/git/archive), prompt injection (`formatSkillsForPrompt`), and optional tool dispatch. |
| **Decision** | **IGNORE** prompt-injection surface; optionally **KEEP** install/discovery/dispatch pipeline |
| **Notes** | Skills are prompt content engineered for a model — brain-adjacent. AI-OS should not consume OpenClaw's prompt-injection. The installation/dispatch machinery is reusable runtime and could be re-purposed for AI-OS capability packaging. |

### 2.14 Plugins — `src/plugins/`, `packages/plugin-sdk/`, `packages/plugin-package-contract/`

| Field | Value |
|---|---|
| **Purpose** | Capability registration runtime. `definePluginEntry` → `OpenClawPluginApi` with `registerTool`, `registerProvider`, `registerChannel`, `registerHook`, `registerHttpRoute`, `registerGatewayMethod`, `registerService`, … |
| **Key files** | `src/plugin-sdk/plugin-entry.ts`, `src/plugins/plugin-api.types.ts`, `src/plugins/tools.ts`, `packages/plugin-sdk/` |
| **Decision** | **KEEP** |
| **Notes** | This is the extension mechanism that makes OpenClaw's connector ecosystem pluggable. AI-OS benefits from the *tool* and *channel* registration surface; it ignores provider/hook surfaces that feed the agent loop. |

### 2.15 LLM Transport — `packages/ai/`, `packages/llm-core/`

| Field | Value |
|---|---|
| **Purpose** | Provider transport/streaming layer (`configureAiTransportHost`, `stream.ts`) and core LLM types (`Message`, `Model`, `ToolResultMessage`, event-stream, validation). |
| **Decision** | **KEEP** as an LLM adapter option |
| **Notes** | AI-OS already owns an Intelligence coordinator stack (`intelligence/`). If that stack needs additional provider transport, OpenClaw's is a candidate — never required. |

### 2.16 Extensions catalog — `extensions/` (161 dirs)

| Category | Examples | Decision |
|---|---|---|
| Model providers | anthropic, google, deepseek, groq, openrouter, vllm, ollama, … | **IGNORE** (AI-OS uses its own intelligence stack) |
| Channels | telegram, whatsapp, slack, signal, discord, imessage, msteams, matrix, … | **KEEP** (communication runtime) |
| Tools/capabilities | browser (CDP), cua-computer, canvas, document-extract, web-readability, firecrawl, exa, tavily, brave, deepgram, elevenlabs, … | **KEEP** (capabilities on demand) |
| Memory | active-memory, memory-core, memory-lancedb, memory-wiki, vault | **REMOVE FROM FLOW** (cognition) |
| Observability/ops | diagnostics-otel, diagnostics-prometheus, admin-http-rpc, webhooks, device-pair | **KEEP** (ops) |
| Email | **absent** | **GAP** — see §0 and `email-send-integration.md` |

### 2.17 UI / Apps — `ui/`, `apps/`

| Field | Value |
|---|---|
| **Purpose** | Browser Control UI (React) and companion apps (macOS/iOS/Android). |
| **Decision** | **IGNORE** (initial integration) |
| **Notes** | May become optional AI-OS surface later. Out of scope for the Runtime integration. |

---

## 3. Key Properties AI-OS Depends On

1. **Tools are callable without the agent loop.** `AgentTool.execute(...)` is a plain async function. AI-OS (via the MCP boundary) can invoke capabilities with zero OpenClaw cognition.
2. **Channels are pluggable transports.** One uniform `ChannelPlugin` contract spans 20+ messengers. Adding/removing a channel is configuration + extension, never AI-OS code change.
3. **MCP is a generic boundary.** `tools/list` / `tools/call` over stdio or HTTP means OpenClaw is swappable for any MCP-capable runtime (Home Assistant, ROS, native runtime) without touching AI-OS.
4. **All cognition lives in OpenClaw subsystems that are cleanly bypassable.** The Gateway can host channels/tools while the reasoning loop (`runEmbeddedAgent`, system-prompt, compaction, memory) is never invoked by AI-OS.

## 4. Risks & Gaps

| Risk | Impact | Mitigation |
|---|---|---|
| OpenClaw has no email channel | `email.send` first integration blocked on built-in connector | Implement email as an AI-OS Runtime-API connector (`runtime-api` side), or build an OpenClaw channel extension per the `ChannelPlugin` contract. Recommended: Runtime-API connector (see `email-send-integration.md`). |
| OpenClaw's Gateway ships its own prompt builder / system-agent | Could accidentally run OpenClaw cognition | AI-OS drives the Gateway only through explicit RPC/MCP with its own prompts; system-agent onboarding flow is never started. |
| Version drift between OpenClaw RPC protocol and adapter | Breakage on OpenClaw updates | Prefer the stable MCP boundary over the raw WS RPC; keep all OpenClaw-specific translation inside `openclaw-runtime`. |
| OpenClaw updates may change tool schemas | Adapter fragility | The `runtime-api` maps capabilities at the boundary; schema changes are absorbed by the adapter only. |
| Node.js runtime dependency | AI-OS must run and supervise a Node process | OpenClaw runs as a supervised sidecar service (systemd/Runtime Supervisor); failure isolation via the existing Execution Platform sandboxing. |
