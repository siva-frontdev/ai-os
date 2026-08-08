# WhatsApp Provider

**Status:** Production (implemented; outbound verified against the Cloud API
contract, inbound via verified webhook)
**Capabilities:** `whatsapp.send`, `whatsapp.receive`
**Plugin:** `whatsapp` (`runtime/plugins/src/whatsapp.rs`)
**Provider:** `WhatsAppProvider` (`runtime/plugins/src/provider/whatsapp.rs`)
**Webhook:** `WebhookServer` (`runtime/plugins/src/provider/webhook.rs`)

The WhatsApp plugin delivers outbound messages through the **Meta WhatsApp
Cloud API** and receives inbound messages through a **webhook** that feeds
the runtime as observations.

---

## Outbound Flow (`whatsapp.send`)

1. `POST {graph_base}/{api_version}/{phone_number_id}/messages` with a
   Bearer access token.
2. Success is reported **only** when Meta returns a message id
   (`messages[0].id`).

## Inbound Flow (`whatsapp.receive`)

1. Meta delivers inbound messages to the configured webhook URL.
2. The webhook listener verifies the request:
   - **GET** (subscription verification): `hub.verify_token` must match
     `AIOS_WHATSAPP_WEBHOOK_VERIFY_TOKEN`; returns `hub.challenge`.
   - **POST** (messages): `X-Hub-Signature-256` must match an HMAC-SHA256 of
     the body computed with `AIOS_WHATSAPP_APP_SECRET`.
3. Verified messages are queued; the runtime calls `whatsapp.receive` to
   drain them into `inbound_message` observations (sender, text, message id,
   timestamp) that reach the Cognitive Loop.

The webhook listener starts only when `AIOS_WHATSAPP_WEBHOOK_PORT` is set.

---

## Setup

The fastest path is the `life` wizard, which validates the credentials
against the live Meta API and optionally sends a real test message:

```bash
cargo run -p life -- setup whatsapp \
  --env-file runtime/config/.env \
  --to "+15551234567"
```

Manual setup:

1. **Meta for Developers** → create an app → add the **WhatsApp product**.
2. Link a **Business Portfolio / WhatsApp Business Account** and register a
   **phone number** that has never been used with WhatsApp.
3. **System user** with `whatsapp_business_messaging` + `whatsapp_business_management`
   permissions → generate a **long-lived access token**.
4. Note the **Phone Number ID** (WhatsApp > API Setup) and **App Secret**
   (App Settings > Basic).
5. Set the webhook **Verify token** to any secret string you choose.
6. Configure the webhook in the app: callback URL
   `https://<host>:<port>/webhook/whatsapp`, verify token, subscribe to the
   **messages** field.
7. **Subscribe the phone number** to the app's webhook.

The callback URL must be **public HTTPS** (Meta requires TLS). In a local dev
setup, expose the webhook port via a tunnel (e.g. `cloudflared tunnel` or
`ngrok`).

Sandbox note: when the WhatsApp account is still in the Meta **sandbox**
environment, the recipient phone number must be added to the sandbox
recipient list in the Meta app before delivery succeeds. The wizard's
`--to <phone>` test message will fail with error `(#131030) Recipient phone
number not in allowed list` until the recipient is added.

---

## Environment Variables

| Variable | Required | Default | Purpose |
|---|---|---|---|
| `AIOS_WHATSAPP_ACCESS_TOKEN` | yes | — | Long-lived system-user token |
| `AIOS_WHATSAPP_PHONE_NUMBER_ID` | yes | — | Phone number id that sends/receives |
| `AIOS_WHATSAPP_API_VERSION` | no | `v21.0` | Graph API version |
| `AIOS_WHATSAPP_GRAPH_BASE` | no | `https://graph.facebook.com` | Graph base (overridable for testing) |
| `AIOS_WHATSAPP_WEBHOOK_VERIFY_TOKEN` | no | — | Token checked on webhook GET verification |
| `AIOS_WHATSAPP_APP_SECRET` | no | — | HMAC secret for `X-Hub-Signature-256` |
| `AIOS_WHATSAPP_WEBHOOK_HOST` | no | `127.0.0.1` | Webhook bind interface |
| `AIOS_WHATSAPP_WEBHOOK_PORT` | no | `0` (disabled) | Webhook listen port |
| `AIOS_WHATSAPP_WEBHOOK_PATH` | no | `/webhook/whatsapp` | Webhook URL path |

The `whatsapp` plugin activates the real provider only when the two required
variables are present. Without them both tools return:

```json
{"error": {"code": "ConfigurationMissing", "message": "WhatsApp provider is not configured: ...", "retryable": false}}
```

---

## Validation

### Unconfigured (no credentials)

```bash
cargo run -p ai-os-plugins --bin ai-os-mcp-server -- --plugins whatsapp
# whatsapp.send / whatsapp.receive → ConfigurationMissing
```

### Configured — outbound

```bash
export AIOS_WHATSAPP_ACCESS_TOKEN=...
export AIOS_WHATSAPP_PHONE_NUMBER_ID=...

# Call whatsapp.send via any MCP client with
#   { "to": "+15551234567", "text": "hello" }
# Then confirm the message arrives on the recipient's WhatsApp.
```

### Configured — inbound

```bash
export AIOS_WHATSAPP_ACCESS_TOKEN=...
export AIOS_WHATSAPP_PHONE_NUMBER_ID=...
export AIOS_WHATSAPP_APP_SECRET=...
export AIOS_WHATSAPP_WEBHOOK_VERIFY_TOKEN=secret
export AIOS_WHATSAPP_WEBHOOK_PORT=8443

# Point Meta's webhook callback at https://<host>:8443/webhook/whatsapp,
# subscribe the phone number, then send a message to the number.
# Call whatsapp.receive → the message is returned as an inbound_message
# observation (source "whatsapp.receive").
```

### Automated

- `cargo test -p ai-os-plugins` covers webhook parsing/queuing, GET
  verification, HMAC signature verification (RFC 4231 reference vector), and
  the honest `ConfigurationMissing` paths.
- `tests/cognitive-integration` and `tests/runtime-validation` dispatch
  `whatsapp.send` through the real subprocess and assert the structured
  failure when unconfigured.

---

## Error Taxonomy

| Code | When | Retryable |
|---|---|---|
| `ConfigurationMissing` | Required env vars absent | no |
| `AuthenticationRequired` | Meta rejects the token (HTTP 401/403) | no |
| `ProviderUnavailable` | Network failure or Meta 5xx | yes |
| `Timeout` | Request timed out | yes |
| `InvalidInput` | Empty `to` or empty `text` | no |
| `ProviderError` | Malformed response, or no message id returned | no |

---

## Troubleshooting

- **Webhook verification (GET) fails** — the verify token in the Meta app
  must equal `AIOS_WHATSAPP_WEBHOOK_VERIFY_TOKEN`, and the callback URL path
  must match `AIOS_WHATSAPP_WEBHOOK_PATH`.
- **`Signature verification failed` in logs** — `AIOS_WHATSAPP_APP_SECRET`
  must equal the app secret shown in Meta (App Settings > Basic). Disabling
  the secret skips verification — do not do this in production.
- **`AuthenticationRequired` on send** — regenerate the long-lived token;
  system-user tokens expire.
- **Messages not arriving** — confirm the phone number is **subscribed** to
  the app webhook and the app has the `whatsapp_business_messaging`
  permission; the webhook URL must be publicly reachable over HTTPS.
- **`ConfigurationMissing` despite env set** — export the variables into the
  `ai-os-mcp-server` process; verify with `env | grep AIOS_WHATSAPP`.

---

## References

- `runtime/plugins/src/provider/whatsapp.rs` — outbound + webhook ingestion
- `runtime/plugins/src/provider/webhook.rs` — HTTP listener, HMAC verification
- `runtime/plugins/src/whatsapp.rs` — `whatsapp.send` / `whatsapp.receive`
- `runtime/plugins/src/provider/config.rs` — env loading
- `docs/runtime/runtime-capability-audit.md` — capability status
