# Telegram Channel

AI-OS can communicate through Telegram using the same cognitive pipeline as the Web UI. The Brain never knows the message originated from Telegram.

## Architecture

```
Telegram User
     │
     ▼
Telegram Bot API (long polling)
     │
     ▼
TelegramAdapter (companion_host/telegram.rs)
     │
     ├── sendChatAction("typing") while processing
     │
     ▼
CognitiveLoopService::cycle()  ← same as Web UI
     │
     ├── World Understanding (LLM)
     ├── World Model Update
     ├── Attention Evaluation
     ├── Decision
     │
     ▼
Decision::Communicate → sendMessage → Telegram User
```

No Brain code was modified. No Cognitive Loop code was modified. No World Understanding code was modified.

## Creating a Bot with BotFather

1. Open Telegram and search for **@BotFather**.
2. Send `/newbot`.
3. Follow the prompts:
   - Choose a display name (e.g. `My AI Companion`)
   - Choose a username ending in `bot` (e.g. `MyAICompanionBot`)
4. BotFather will respond with a bot token:

   ```
   Use this token to access the HTTP API:
   1234567890:ABCdefGHIjklMNOpqrsTUVwxyzABCDEFGH
   ```

5. (Optional) Set a description: `/setdescription`
6. (Optional) Set a profile photo: `/setuserpic`

## Configuration

### settings.json

Add the `telegram` section to `~/.config/ai-os-companion/settings.json`:

```json
{
  "telegram": {
    "enabled": true,
    "bot_token": "1234567890:ABCdefGHIjklMNOpqrsTUVwxyzABCDEFGH",
    "poll_interval_secs": 2
  }
}
```

| Field | Type | Default | Description |
|---|---|---|---|
| `enabled` | bool | `false` | Start Telegram polling at launch |
| `bot_token` | string | `""` | Telegram Bot API token |
| `poll_interval_secs` | integer | `2` | Seconds between getUpdates polls |

### Environment Variable

The bot token can be overridden with the `AI_OS_TELEGRAM_BOT_TOKEN` environment variable. This takes priority over the settings file value.

```bash
export AI_OS_TELEGRAM_BOT_TOKEN="1234567890:ABCdefGHIjklMNOpqrsTUVwxyzABCDEFGH"
```

This is useful when you want to keep the token out of the config file, or when running in CI / headless mode.

## Running

The Telegram channel starts automatically when:

1. `telegram.enabled` is `true` in settings **AND** a bot token is available (from settings or env var)
2. Or the `AI_OS_TELEGRAM_BOT_TOKEN` env var is set

Start the companion as usual:

```bash
AI_OS_TELEGRAM_BOT_TOKEN="your-token" ./target/release/desktop-companion
```

Expected log output:

```
INFO desktop_companion: Telegram channel configured
INFO brain_coordinator::companion_host::telegram: Telegram bot @YourBot connected
INFO brain_coordinator::companion_host::telegram: Telegram polling started (interval: 2s)
```

If the token is invalid, the companion will log an error and continue without Telegram:

```
ERROR brain_coordinator::companion_host::telegram: Telegram bot token invalid: ...
```

## User Identity Mapping

Each Telegram user who sends a message gets a persistent identity stored at:

```
~/.local/share/ai-os-companion/telegram_identities.json
```

The file contains an array of:

```json
[
  {
    "telegram_user_id": 123456789,
    "username": "johndoe",
    "display_name": "John",
    "first_seen": "2026-07-29T16:30:00+00:00",
    "last_active": "2026-07-29T17:00:00+00:00"
  }
]
```

Messages from the same Telegram account continue the same conversation and update the same World Model.

## Conversation Flow

1. User sends a message to the bot on Telegram.
2. The bot sends a "typing" indicator while processing.
3. The message goes through `CognitiveLoopService::cycle()` — exactly the same pipeline as the Web UI.
4. World Understanding extracts entities and updates the World Model.
5. Attention evaluates the context and decides whether to communicate.
6. If the decision is `Communicate`, the response is sent as a Telegram reply.
7. Telegram markdown characters are escaped to prevent rendering errors.

## Troubleshooting

### Bot does not respond

1. Verify the token is correct: `curl https://api.telegram.org/bot<TOKEN>/getMe`
2. Check the companion logs for `Telegram bot @... connected`.
3. Ensure the user has started a chat with the bot (send `/start`).
4. Check that the bot is not blocked by the user.

### "Telegram bot token invalid" error

1. Verify the token in settings or env var.
2. Regenerate the token with BotFather (`/revoke`).
3. Ensure there are no extra spaces or newlines in the token.

### Bot responds slowly

1. Decrease `poll_interval_secs` in settings (minimum 1).
2. The cognitive pipeline takes ~2 seconds for an NVAPI LLM call.
3. Total latency = poll_interval + LLM_time + API_roundtrip.

### Messages not being picked up

1. The bot uses long polling with `timeout=10` seconds per request.
2. Network issues can cause missed polls — check logs for `Telegram poll request failed`.
3. If the companion was restarted, the `offset` resets and recent messages may be re-processed.

## Developer Diagnostics

When `developer_mode` is enabled in settings, the Web UI shows a Telegram diagnostics panel with:

- Connection status
- Number of polls executed
- Number of updates received
- Messages processed
- Last received/sent timestamps
- Current chat ID
- Average response latency
- Error count

## Common Errors

| Error | Cause | Fix |
|---|---|---|
| `Telegram getMe returned ok=false` | Invalid bot token | Check token with BotFather |
| `Telegram poll request failed` | Network / timeout | Check internet connectivity |
| `Telegram sendMessage failed` | Invalid chat ID / network | Usually transient; check logs |
| `invalid type: map, expected a string` | LLM returned structured observation instead of string | World Understanding prompt issue (not Telegram-specific) |