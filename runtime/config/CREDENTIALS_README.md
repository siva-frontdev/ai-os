# Runtime Configuration Directory

This directory holds runtime-local configuration that must **never** be
committed to the repository.

## Files

| File | Purpose | Git status |
|---|---|---|
| `.env` | Provider credentials written by `life setup <provider>` | Ignored (`.gitignore` matches `.env`) |
| `CREDENTIALS_README.md` | This file | Tracked |

## Credentials

The `.env` file stores OAuth2 client credentials and refresh tokens for the
provider plugins (Gmail, and in future WhatsApp, GitHub, Calendar, Telegram).
It is written by the `life setup` wizard with **owner-only permissions**
(`0600`) and is ignored by Git.

### Security rules

- Never commit `.env`. `.gitignore` already excludes it.
- Never log credential values. Logs must reference keys only.
- The refresh token is long-lived; the access token is **never** stored.
- Rotate credentials in the Google Cloud console if the file is ever exposed.

### Why `.env` (for now)

The platform roadmap calls for OS keychain/encrypted credential storage
(Phase 4 OSAL). Until that lands, `.env` behind `.gitignore` with `0600`
permissions is the pragmatic, documented home for secrets.

## How to populate

```bash
# From the workspace root — complete the OAuth flow (browser, no OAuth Playground):
cargo run -p life -- setup gmail
```

The wizard stores `AIOS_GMAIL_CLIENT_ID`, `AIOS_GMAIL_CLIENT_SECRET` and
`AIOS_GMAIL_REFRESH_TOKEN`, then validates the provider end-to-end.

See `docs/runtime/gmail-setup.md` for the full walkthrough.
