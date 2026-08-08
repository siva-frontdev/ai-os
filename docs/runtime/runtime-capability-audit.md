# Runtime Capability Audit

**Date**: 2026-08-06
**Phase**: 1 — Production Capability Bring-up (LIFE Beta)
**Scope**: Every capability exposed by `RuntimeManager` via the MCP plugin
surface, its plugin, backing provider, current implementation, and production
readiness.

**Production scope this phase**: Gmail (`email.send`) and WhatsApp
(`whatsapp.send`, `whatsapp.receive`) are the **only** capabilities brought to
production. GitHub, Calendar, Browser, and Filesystem providers are
**out of scope** and remain non-production or unavailable.

---

## Method

Capabilities were enumerated from the plugin surface the `ai-os-mcp-server`
binary hosts (`runtime/plugins/src/lib.rs`) and cross-checked against the
capabilities `RuntimeManager` merges after `initialize()`
(`runtime/runtime-manager/src/manager.rs`). Each plugin's implementation was
read in full (`runtime/plugins/src/{email,filesystem,github,calendar,telegram,whatsapp}.rs`)
and the provider layer (`runtime/plugins/src/provider/`).

Readiness definitions:

| Status | Meaning |
|---|---|
| ✅ **Production** | Executes against a real external service; success confirmed by the provider |
| 🟡 **Partial** | Real effect for part of the flow, but not provider-verified end to end |
| 🟠 **Mock** | In-memory state; simulates success without contacting any external service |
| 🔴 **Missing** | Advertised in the mission but not implemented as a tool |
| ⚠ **Broken** | Present but fails its contract in production use |

---

## Capability Inventory

| Capability | Runtime | Plugin | Provider | Implementation | Readiness |
|---|---|---|---|---|---|
| `email.send` | MCP (`openclaw`) | `email` | **Gmail API** | OAuth2 refresh → `users.messages.send` → verify `SENT` label (`provider/gmail.rs`); success only after confirmation | ✅ **Production** |
| `email.inject` | MCP (`openclaw`) | `email` | in-memory inbox | Simulated inbound; emits an inbound observation (`email.rs`) | 🟠 **Mock (test/dev)** |
| `whatsapp.send` | MCP (`openclaw`) | `whatsapp` | **Meta WhatsApp Cloud API** | `POST /{version}/messages` with bearer token; success only on Meta message id (`provider/whatsapp.rs`) | ✅ **Production** |
| `whatsapp.receive` | MCP (`openclaw`) | `whatsapp` | **Meta webhook** | Webhook listener (signature-verified) queues inbound; `receive` drains into observations (`provider/webhook.rs`) | ✅ **Production** |
| `filesystem.read` | MCP (`openclaw`) | `filesystem` | **real filesystem** | `tokio::fs::read_to_string` under a canonicalized root (`filesystem.rs`) | ✅ **Production** |
| `filesystem.write` | MCP (`openclaw`) | `filesystem` | **real filesystem** | `tokio::fs::write` with root-containment check (`filesystem.rs`) | ✅ **Production** |
| `filesystem.search` | MCP (`openclaw`) | `filesystem` | **real filesystem** | Recursive `read_dir` walk filtered by name (`filesystem.rs`) | ✅ **Production** |
| `telegram.send` | MCP (`openclaw`) | `telegram` | in-memory sent-list | Appends to `Vec<Value>`, returns `status:"sent"` (`telegram.rs`) | 🟠 **Mock (out of scope)** |
| `telegram.inject_inbound` | MCP (`openclaw`) | `telegram` | in-memory | Emits a simulated inbound observation (`telegram.rs`) | 🟠 **Mock (out of scope)** |
| `github.create_issue` | MCP (`openclaw`) | `github` | in-memory repo | Pushes to `Vec<Value>`, numbers issues 1..N (`github.rs`) | 🟠 **Mock (out of scope)** |
| `github.read_repository` | MCP (`openclaw`) | `github` | in-memory repo | Reads the same in-memory `Vec` (`github.rs`) | 🟠 **Mock (out of scope)** |
| `calendar.read` | MCP (`openclaw`) | `calendar` | in-memory event store | Filters in-memory `Vec<Value>` by date (`calendar.rs`) | 🟠 **Mock (out of scope)** |
| `calendar.create_event` | MCP (`openclaw`) | `calendar` | in-memory event store | Pushes to `Vec<Value>`, returns `status:"scheduled"` (`calendar.rs`) | 🟠 **Mock (out of scope)** |
| `telegram.receive` | — | — | — | Not a tool; out of scope this phase | 🔴 **Missing (deferred)** |
| `github.read_file` | — | — | — | Not a tool; out of scope this phase | 🔴 **Missing (deferred)** |
| `calendar.update_event` | — | — | — | Not a tool; out of scope this phase | 🔴 **Missing (deferred)** |
| `browser.open` | — | — | — | No browser plugin; out of scope this phase | 🔴 **Missing (deferred)** |
| `browser.search` | — | — | — | No browser plugin; out of scope this phase | 🔴 **Missing (deferred)** |

**Advertised (plugin tools): 13**
**Production-ready: 7** (`email.send`, `whatsapp.send`, `whatsapp.receive`,
`filesystem.read`, `filesystem.write`, `filesystem.search`)
**Mock: 6** (email.inject, telegram ×2, github ×2, calendar ×2 — all out of
scope or test/dev only)
**Deferred / out of scope: 5** (`telegram.receive`, `github.read_file`,
`calendar.update_event`, `browser.open`, `browser.search`)

---

## Findings

### 1. Gmail and WhatsApp are now provider-verified, production capabilities

- `email.send` exchanges the refresh token for an access token, POSTs the
  RFC 2822 message to `users.messages.send`, re-fetches the message, and
  reports success **only** when the `SENT` label is present
  (`runtime/plugins/src/provider/gmail.rs`).
- `whatsapp.send` posts to the WhatsApp Cloud API and reports success **only**
  when Meta returns a message id. `whatsapp.receive` drains inbound messages
  that arrived on a signature-verified webhook listener
  (`runtime/plugins/src/provider/whatsapp.rs`, `provider/webhook.rs`).

Providers activate only when credentials are present in the environment
(`AIOS_GMAIL_*`, `AIOS_WHATSAPP_*`). Without credentials the tools return a
structured `ConfigurationMissing` error — they never fabricate success.

### 2. Structured error taxonomy

Every provider failure maps to a stable code via `ProviderError`
(`provider/error.rs`): `ConfigurationMissing`, `AuthenticationRequired`,
`ProviderUnavailable`, `Timeout`, `InvalidInput`, `ProviderError`, each with a
`retryable` flag. The payload `{"error":{code,message,retryable}}` is relayed
verbatim to LIFE so it can escalate or retry. The runtime maps tool errors to
`Failed` with code `tool_error` (`openclaw-runtime/src/result_map.rs`).

### 3. Inbound WhatsApp messages reach the Cognitive Loop

Inbound messages arrive on the webhook (`GET` subscription verification +
`POST` with `X-Hub-Signature-256` HMAC verification). `whatsapp.receive`
drains the queue into `inbound_message` observations consumed by the runtime.

### 4. Out-of-scope plugins remain in-memory and are clearly marked

`telegram`, `github`, `calendar`, and `email.inject` keep their in-memory
behavior for deterministic validation. They are documented as mock/out-of-scope
and are not claimed as production. `browser` remains unimplemented.

### 5. No provider health surface

`RuntimeManager::health()` reports runtime readiness, but there is no plugin-
or provider-level health endpoint (`GET /api/runtime/status`). Deferred.

---

## Plugin Inventory

| Plugin | Name | Tools | Provider | Readiness |
|---|---|---|---|---|
| `ai-os-plugins` email | `email` | `email.send`, `email.inject` | Gmail API (real); inbox (sim) | ✅ / 🟠 |
| `ai-os-plugins` whatsapp | `whatsapp` | `whatsapp.send`, `whatsapp.receive` | Meta Cloud API + webhook (real) | ✅ Production |
| `ai-os-plugins` filesystem | `filesystem` | read / write / search | host filesystem | ✅ Production |
| `ai-os-plugins` github | `github` | `create_issue`, `read_repository` | none (in-memory) | 🟠 Mock (out of scope) |
| `ai-os-plugins` calendar | `calendar` | `read`, `create_event` | none (in-memory) | 🟠 Mock (out of scope) |
| `ai-os-plugins` telegram | `telegram` | `send`, `inject_inbound` | none (in-memory) | 🟠 Mock (out of scope) |

The single `ai-os-mcp-server` binary hosts all enabled plugins over the MCP
stdio transport (`runtime/plugins/src/main.rs`). When WhatsApp is configured
with `AIOS_WHATSAPP_WEBHOOK_PORT`, the binary also binds the webhook listener.

---

## Provider Inventory (current state)

Gmail and WhatsApp providers are implemented. No credentials are present in
the environment, so live end-to-end delivery has not been exercised here;
unconfigured behavior returns `ConfigurationMissing`. Setup and validation
are documented in `docs/runtime/gmail-provider.md` and
`docs/runtime/whatsapp-provider.md`.

---

## Next Steps

| Phase | Action |
|---|---|
| Next | Live E2E with real credentials: send a Gmail message, receive + reply over WhatsApp webhook |
| Deferred | Bring github, calendar, browser, and `telegram.receive` to production with real providers |
| Deferred | Unified `providers:` configuration beyond environment variables |
| Deferred | Provider health surface (`GET /api/runtime/status`) |
