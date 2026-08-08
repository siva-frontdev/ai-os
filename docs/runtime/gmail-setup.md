# Gmail Setup — `life setup gmail`

**Status:** Production (live-validated 2026-08-06)
**Capability:** `email.send`
**Wizard:** `tools/life` — `life setup gmail`

This guide walks through connecting a real Gmail account to AI-OS using the
`life` CLI wizard. The wizard runs the **real OAuth2 flow** in your browser —
no OAuth Playground, no manual `curl` — and stores the resulting credentials
in `runtime/config/.env` (gitignored, owner-only permissions).

---

## 1. Google Cloud project

1. Go to <https://console.cloud.google.com> and create (or pick) a project.
2. Enable the **Gmail API**:
   Console → *APIs & Services → Library* → search "Gmail API" → **Enable**.
3. **Create an OAuth client**:
   *APIs & Services → Credentials → Create Credentials → OAuth client ID*.
   - Application type: **Desktop app**.
   - Copy the **Client ID** and **Client Secret**.

### Authorized redirect URI

The wizard's localhost callback server listens at
`http://127.0.0.1:8765/oauth/google/callback`.

- **Desktop app** clients accept the `127.0.0.1` loopback redirect without
  additional configuration in most setups.
- If Google rejects the redirect (`redirect_uri_mismatch`), add
  `http://127.0.0.1:8765/oauth/google/callback` under *Authorized redirect
  URIs* on the OAuth client. If you pass a different `--port`, register that
  exact URI instead.

### Scope

The wizard requests two scopes:

- `https://www.googleapis.com/auth/gmail.send` — send mail only.
- `https://www.googleapis.com/auth/gmail.metadata` — read message metadata
  (headers and labels only, never body content). The provider uses this to
  re-fetch the just-sent message and confirm the `SENT` label before
  reporting success (`GmailProvider::verify_sent`).

No inbox reading beyond metadata is requested.

If you set up Gmail before `gmail.metadata` was added, re-run
`life setup gmail` and approve the consent again — Google will re-issue the
refresh token with both scopes.

---

## 2. Pre-load the client credentials

The wizard needs the Client ID and Secret before it can build the consent
URL. Put them in `runtime/config/.env` (already gitignored):

```bash
AIOS_GMAIL_CLIENT_ID=<CLIENT_ID>
AIOS_GMAIL_CLIENT_SECRET=<CLIENT_SECRET>
```

or export them in the shell:

```bash
export AIOS_GMAIL_CLIENT_ID=<CLIENT_ID>
export AIOS_GMAIL_CLIENT_SECRET=<CLIENT_SECRET>
```

---

## 3. Run the wizard

From the workspace root:

```bash
cargo run -p life -- setup gmail
```

What happens:

1. **Consent** — the wizard starts a localhost callback server
   (`127.0.0.1:8765`), opens the Google consent screen in your browser, and
   waits.
2. **Authorize** — sign in as the account that will send mail, review the
   `gmail.send` scope, and allow.
3. **Callback** — Google redirects to
   `http://127.0.0.1:8765/oauth/google/callback?code=...&state=...`. The
   wizard validates `state` (CSRF protection), exchanges the code at
   `https://oauth2.googleapis.com/token`, and receives a long-lived
   **refresh token**.
4. **Persist** — writes to `runtime/config/.env`:
   - `AIOS_GMAIL_CLIENT_ID`
   - `AIOS_GMAIL_CLIENT_SECRET`
   - `AIOS_GMAIL_REFRESH_TOKEN`
   - `AIOS_GMAIL_USER` (the validated account, when available)

   The short-lived **access token is never persisted**.
5. **Validate** — the wizard constructs a real `GmailProvider` from the
stored config and exchanges the refresh token for an access token. A
successful token exchange proves the client id, client secret, and refresh
token are valid and authorized. Only then does it report success.

A live `users.profile` lookup requires a read scope (e.g. `gmail.readonly`
or `gmail.metadata`) which is outside the minimal `gmail.send` scope the
wizard requests; the wizard therefore does **not** auto-populate
`AIOS_GMAIL_USER`. The sender defaults to `me`, which the Gmail API
resolves to the authenticated account automatically.

### Options

| Flag | Meaning |
|---|---|
| `--env-file <path>` | credentials file (default `runtime/config/.env`) |
| `--port <n>` | localhost callback port (default `8765`; `0` = auto) |
| `--timeout <secs>` | seconds to wait for the callback (default `300`) |
| `--to <email>` | send a real test email after validation |
| `--no-verify` | skip live validation |
| `--force` | re-run OAuth even if a refresh token is already stored |
| `--list` | list registered providers |

Example with a test email:

```bash
cargo run -p life -- setup gmail --to you@example.com
```

---

## 4. Verify the runtime can send

Once configured, the plugin binary picks the credentials up from
`runtime/config/.env` exactly as `life` wrote them. Run the MCP server and
call `email.send` through any MCP client, or run the automated suite:

```bash
cargo test -p ai-os-plugins
```

The integration tests in `tools/life/tests/gmail_setup.rs` run the entire
wizard flow (callback server → code exchange → env persistence → provider
validation) against a local stand-in Google backend.

---

## Error handling

Every failure surfaces as a **structured error** (same JSON shape as the
runtime's `ProviderError`), never a silent success:

```json
{"error": {"code": "AuthenticationRequired", "message": "...", "retryable": false}}
```

| Code | When | Retryable |
|---|---|---|
| `ConfigurationMissing` | Client id/secret absent before the flow | no |
| `ProviderUnavailable` | Callback server bind failure, browser open failure, token network failure | yes |
| `AuthenticationRequired` | Consent denied, callback carried `error=`, token exchange rejected | no |
| `Timeout` | No callback within `--timeout` seconds | yes |
| `ProviderError` | State mismatch (possible CSRF), persistence failure, validation failure | no |

---

## Troubleshooting

- **Browser never opens** — the wizard prints the consent URL; paste it into
  a browser manually.
- **`redirect_uri_mismatch`** — add the exact redirect URI (including port)
  to the OAuth client's authorized redirect URIs.
- **Token exchange returns `invalid_grant`** — client/secret mismatch, or the
  code was already consumed (re-run; never reuse a code).
- **No refresh token in the exchange** — the client must consent with
  `access_type=offline&prompt=consent` (the wizard always uses both); ensure
  you did not reuse an already-granted code from a prior flow.
- **`AuthenticationRequired` at validation** — the refresh token was revoked
or the client id/secret mismatch; re-run `life setup gmail --force`.
- **Port 8765 busy** — pass `--port 0` (auto) or another port and register
  the matching redirect URI.

---

## References

- `tools/life/src/setup/gmail.rs` — the Gmail wizard
- `tools/life/src/setup/oauth.rs` — callback server, browser open, token exchange
- `tools/life/src/setup/env_file.rs` — secure `.env` persistence
- `tools/life/tests/gmail_setup.rs` — end-to-end wizard tests
- `docs/runtime/gmail-provider.md` — provider delivery flow and error taxonomy
- `runtime/config/CREDENTIALS_README.md` — credential storage rules
