# AI-OS Companion

The AI-OS Companion is a desktop application that provides a personal AI assistant
integrated with the operating system. It runs as a background service with a
web-based UI and optional system tray integration.

## Architecture

```
┌─────────────────────────────────────────────────────┐
│                  User Desktop                        │
│  ┌──────────────┐  ┌────────────────────────────┐   │
│  │ Web Browser  │  │  Desktop Companion (tray)   │   │
│  │ (localhost)  │  │  notify-send / autostart    │   │
│  └──────┬───────┘  └────────────────────────────┘   │
│         │                 │                          │
└─────────┼─────────────────┼──────────────────────────┘
          │                 │
          ▼                 ▼
┌─────────────────────────────────────────────────────┐
│              Companion Host (brain-coordinator)      │
│  ┌──────────┐ ┌──────────┐ ┌────────────────────┐  │
│  │ Web UI   │ │Cog Loop  │ │ World Model        │  │
│  │ (warp)   │ │(attention│ │ (entities/rels)    │  │
│  │          │ │+reflect) │ │                    │  │
│  └──────────┘ └──────────┘ └────────────────────┘  │
│  ┌──────────┐ ┌──────────┐ ┌────────────────────┐  │
│  │ Settings │ │Observers │ │ Persistence        │  │
│  │ Manager  │ │(sources) │ │ (JSON files)       │  │
│  └──────────┘ └──────────┘ └────────────────────┘  │
└─────────────────────────────────────────────────────┘
```

### Components

| Component | Crate | Responsibility |
|---|---|---|
| `brain-coordinator` (lib) | `brain/brain-coordinator/` | Companion host: cognitive loop, world model, web UI, settings |
| `desktop-companion` (bin) | `apps/desktop-companion/` | Desktop entry point: tray, notifications, autostart, signal handling |

## Installation

### From source

```bash
# Build the companion host library and desktop binary
cargo build --release -p desktop-companion

# The binary is at target/release/desktop-companion
```

### System-wide install

```bash
sudo cp target/release/desktop-companion /usr/local/bin/
```

## Configuration

The companion reads settings from `~/.config/ai-os-companion/settings.json`.
A default file is created automatically on first run.

### Settings reference

| Key | Type | Default | Description |
|---|---|---|---|
| `llm_provider` | string | `"openai"` | LLM provider (`openai`, `anthropic`, `ollama`, `custom`) |
| `llm_api_key` | string | `""` | API key for the LLM provider |
| `llm_model` | string | `""` | Model identifier |
| `observation.interval_secs` | number | `60` | How often to observe the environment |
| `observation.observation_window` | number | `300` | Sliding window for observation history |
| `observation.linux_processes` | bool | `true` | Observe running processes |
| `observation.linux_system` | bool | `true` | Observe system load |
| `notifications.enabled` | bool | `true` | Enable desktop notifications |
| `notifications.sound` | bool | `true` | Play notification sounds |
| `notifications.quiet_hours_start` | string | `""` | Quiet hours start time (HH:MM) |
| `notifications.quiet_hours_end` | string | `""` | Quiet hours end time (HH:MM) |
| `attention_sensitivity` | number | `0.7` | Sensitivity threshold (0.0–1.0) |
| `reflection_frequency` | number | `5` | Reflection cycles between observations |
| `autostart` | bool | `false` | Start automatically on login |
| `dev_mode` | bool | `false` | Enable developer dashboard tab |
| `debug_logging` | bool | `false` | Enable debug-level logging |
| `wm_storage_path` | string | `~/.local/share/ai-os-companion/wm/` | World Model storage path |
| `ui_port` | number | `3030` | Web UI HTTP port |

## Usage

### Starting the companion

```bash
desktop-companion
```

The web UI is available at `http://localhost:3030`.

### Command-line flags

None currently. All configuration is through `settings.json`.

### Stopping

Press `Ctrl+C` to stop gracefully. The world model is persisted before shutdown.

## Web UI

The companion provides a web-based interface with these tabs:

- **Status** — Overview of the companion's current state, last decision, recent reflections, attention signals
- **World Model** — View entities and relationships in the knowledge graph
- **Chat** — Send messages to the companion and receive responses
- **Reflections** — History of companion reflections (insights, patterns, decisions)
- **Diagnostics** — Raw data tables for debugging
- **Settings** — Modify companion configuration (applies on restart)
- **Dashboard** — Developer view with detailed entity/relationship tables (visible only when `dev_mode` is enabled)

### API endpoints

| Method | Path | Description |
|---|---|---|
| GET | `/api/status` | Companion status (version, cycles, counts, last decision) |
| GET | `/api/entities` | World model entities |
| GET | `/api/relationships` | World model relationships |
| GET | `/api/reflections` | Reflection history |
| GET | `/api/settings` | Current settings |
| POST | `/api/settings` | Update settings |
| POST | `/chat` | Send a chat message |
| GET | `/events` | SSE stream for live updates |

## Desktop Features

### Native notifications

The companion sends native desktop notifications via `notify-send` (Linux).
Notifications are dispatched when the companion decides to communicate
(e.g., reminders, important observations, reflection results).

### Autostart

When `autostart` is enabled in settings, the companion registers a
`.desktop` file in `~/.config/autostart/ai-os-companion.desktop`.

### System tray (future)

System tray integration is planned but requires `tray-icon` or `libappindicator`
and will be added in a future release.

## Upgrade

1. Stop the companion (Ctrl+C)
2. Build or download the new binary
3. Replace the old binary
4. Start the companion again

See [Quick Start Guide](../../quickstart.md) for first-time setup and configuration.
See [Developer Mode](../../developer-mode.md) for introspection and debugging.
See [Beta Guide](../../beta-guide.md) for daily usage and issue reporting.
See [Validation Checklist](../../validation-checklist.md) for real-world verification.
See [Behavior Issue Template](../../behavior-issue-template.md) for reporting issues.

## Recovery

If the companion crashes or is killed:

1. Check `~/.local/share/ai-os-companion/wm/` for world model data.
2. Check logs at the configured logging output.
3. Restart the companion — it will recover the last persisted world model.

The world model is persisted after every cognitive cycle, so at most one
cycle of observations may be lost on crash.

## Development

### Building

```bash
cargo build -p desktop-companion
```

### Running tests

```bash
cargo test -p brain-coordinator
```

### Checking formatting

```bash
cargo fmt --check
```

### Linting

```bash
cargo clippy -p brain-coordinator -p desktop-companion
```
