# Runtime Integration — Documentation Index

Phase 8 · External Runtime Integration (RFC-0007)

AI-OS owns cognition. The runtime owns execution. OpenClaw is one runtime implementation.

## Documents

| Doc | Deliverable | Purpose |
|---|---|---|
| [RFC-0007](../rfc/RFC-0007-runtime-integration.md) | — | The proposal for the Runtime API + OpenClaw adapter |
| [openclaw-analysis.md](openclaw-analysis.md) | 1 | Per-module analysis of OpenClaw: keep / replace / ignore, with reasons |
| [runtime-api.md](runtime-api.md) | 2 | The generic, vendor-free `Runtime` trait and types |
| [mapping.md](mapping.md) | 3 | OpenClaw module → AI-OS layer → decision mapping |
| [openclaw-adapter.md](openclaw-adapter.md) | 4 | Adapter design for `runtime/ai-os-runtime-api` + `runtime/runtime-manager` + `runtime/openclaw-runtime` |
| [email-send-integration.md](email-send-integration.md) | 5 | First end-to-end integration: Telegram → AI-OS → email.send → reply |
| [runtime-capability-audit.md](runtime-capability-audit.md) | — | Production readiness of every runtime capability |
| [gmail-provider.md](gmail-provider.md) | — | Gmail API provider: setup, env vars, validation, troubleshooting |
| [gmail-setup.md](gmail-setup.md) | — | `life setup gmail` wizard: OAuth2 browser flow, persistence, live validation |
| [whatsapp-provider.md](whatsapp-provider.md) | — | WhatsApp Cloud API + webhook provider: setup, env vars, validation, troubleshooting |

## Boundary Summary

- **AI-OS owns:** World Model, World Understanding, Cognitive Loop, Planner, Evolution, Reflection, Attention, Memory, Decision, Communication Logic.
- **Runtime owns:** expose capabilities, execute actions, communicate with external systems, provide observations, manage connectors/plugins.
- **OpenClaw must never:** keep memory, plan, decide, own conversations/prompts/reasoning.
- **AI-OS must never:** import OpenClaw types.

## OpenClaw Reference

- Clone (read-only, untouched): `~/openclaw` — commit `80da6166`.
- Key finding: **OpenClaw has no email channel** — see `email-send-integration.md` §1.

## Status

Accepted & implemented (2026-08-05). Four new workspace crates:

- `runtime/ai-os-runtime-api` — vendor-free `Runtime` trait, types, `DefaultRuntime`, dispatcher, JSON-Schema subset validation.
- `runtime/runtime-manager` — registers runtimes, merges capabilities, routes actions by `CapabilityId`.
- `runtime/openclaw-runtime` — OpenClaw adapter over MCP (stdio subprocess or in-memory duplex), with a `testkit` mock MCP server.
- `tests/runtime-integration` — 10 cross-crate tests covering registration, dispatch, runtime swap, and the email flow.

Cognition (`brain/`, `runtime/ai-os-runtime`) is untouched. Remaining work: app-level wiring, real OpenClaw email channel extension, live gateway smoke.
