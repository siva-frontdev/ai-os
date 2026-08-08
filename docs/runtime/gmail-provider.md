# Gmail Provider

**Status:** Production (implemented, verified against the Gmail API flow)
**Capability:** `email.send`
**Plugin:** `email` (`runtime/plugins/src/email.rs`)
**Provider:** `GmailProvider` (`runtime/plugins/src/provider/gmail.rs`)

The `email.send` capability delivers real mail through the **Gmail API**
(not SMTP). Success is reported **only** after the provider confirms the
message is in the Sent folder.

---

## Delivery Flow

1. **Authenticate** — exchange the OAuth2 refresh token for an access token
   (`POST {token_uri}` with `grant_type=refresh_token`).
2. **Send** — `POST {api_base}/gmail/v1/users/{user}/messages/send` with the
   message encoded as base64url (`URL_SAFE_NO_PAD`) RFC 2822 text.
3. **Verify** — re-fetch the message (`GET .../messages/{id}?format=metadata`)
   and require the `SENT` label.

Steps 2 and 3 must both succeed before `email.send` reports success. If
delivery cannot be confirmed the tool fails with a structured error.

---

## Setup

> **Recommended:** run `life setup gmail` from the workspace root. The wizard
> performs the real OAuth2 browser flow (no OAuth Playground), stores the
> credentials in `runtime/config/.env`, and validates the provider. See
> [`gmail-setup.md`](gmail-setup.md) for the full walkthrough.

Manual alternative:

1. **Google Cloud project** → enable the **Gmail API**.
2. **OAuth client** (Desktop app) → copy the **Client ID** and
   **Client Secret**.
3. **Refresh token** for the sending account (obtained by completing an
   OAuth2 consent flow; the wizard automates this).
4. Make the credentials available to the `ai-os-mcp-server` process — either
   from `runtime/config/.env` (the server binary loads it at startup; this is
   what `life setup` writes) or the process environment.

The `refresh_token` never expires (unless revoked); the provider refreshes
access tokens automatically on every send.

---

## Environment Variables

| Variable | Required | Default | Purpose |
|---|---|---|---|
| `AIOS_GMAIL_CLIENT_ID` | yes | — | OAuth2 client id |
| `AIOS_GMAIL_CLIENT_SECRET` | yes | — | OAuth2 client secret |
| `AIOS_GMAIL_REFRESH_TOKEN` | yes | — | Long-lived OAuth2 refresh token |
| `AIOS_GMAIL_USER` | no | `me` | Account to send as / verify against |
| `AIOS_GMAIL_TOKEN_URI` | no | `https://oauth2.googleapis.com/token` | Token endpoint (overridable for testing) |
| `AIOS_GMAIL_API_BASE` | no | `https://gmail.googleapis.com` | Gmail API base (overridable for testing) |

The `email` plugin activates the real provider only when the three required
variables are present. Without them `email.send` returns:

```json
{"error": {"code": "ConfigurationMissing", "message": "Gmail provider is not configured: ...", "retryable": false}}
```

---

## Validation

### Unconfigured (no credentials)

```bash
# Run the server without AIOS_GMAIL_*; call email.send via any MCP client.
# Expect: is_error=true with code "ConfigurationMissing".
cargo run -p ai-os-plugins --bin ai-os-mcp-server -- --plugins email
```

### Configured (real delivery)

```bash
export AIOS_GMAIL_CLIENT_ID=...
export AIOS_GMAIL_CLIENT_SECRET=...
export AIOS_GMAIL_REFRESH_TOKEN=...
export AIOS_GMAIL_USER=you@example.com

# Send and then verify the message appears in the Sent folder of
# you@example.com (Gmail web UI → Sent).
```

### Automated

- Plugin unit/integration tests (`cargo test -p ai-os-plugins`) exercise the
  full flow against a local stand-in Gmail backend (token refresh → send →
  SENT-label verify), proving the honest-success contract without live
  credentials.
- Cross-crate tests (`tests/cognitive-integration`, `tests/runtime-validation`)
  assert the unconfigured path fails with `ConfigurationMissing`.

---

## Error Taxonomy

| Code | When | Retryable |
|---|---|---|
| `ConfigurationMissing` | Required env vars absent | no |
| `AuthenticationRequired` | Token refresh rejected (401/400) or Gmail 401/403 — token expired/revoked | no |
| `ProviderUnavailable` | Network failure or Gmail 5xx | yes |
| `Timeout` | Request timed out | yes |
| `InvalidInput` | Empty `to` recipient | no |
| `ProviderError` | Malformed response, or message missing the `SENT` label | no |

---

## Troubleshooting

- **`AuthenticationRequired` on send** — re-run the OAuth flow and replace
  `AIOS_GMAIL_REFRESH_TOKEN`; the refresh token may have been revoked or the
  scope `gmail.send` omitted.
- **`ProviderError: message ... missing the SENT label`** — Gmail accepted but
  did not confirm delivery in the metadata fetch; treat as not sent, retry.
- **`ConfigurationMissing` despite env set** — the variables must be exported
  to the `ai-os-mcp-server` process (the same process that runs the plugin);
  verify with `env | grep AIOS_GMAIL`.
- **403 `Invalid grant`** — client/secret mismatch or a web-app-only client;
  use a Desktop app OAuth client.

---

## References

- `runtime/plugins/src/provider/gmail.rs` — provider implementation
- `runtime/plugins/src/email.rs` — `email.send` / `email.inject` tools
- `runtime/plugins/src/provider/config.rs` — env loading
- `docs/runtime/runtime-capability-audit.md` — capability status
