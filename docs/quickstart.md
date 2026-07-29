# AI-OS Quick Start Guide

Get AI-OS running in under 15 minutes on Arch Linux.

---

## Prerequisites

- **OS:** Arch Linux (required — OSAL is Linux-specific)
- **Rust:** 1.80+ (`rustup default stable`)
- **Packages:** `git`, `curl`, `jq`, `notify-send` (all in `base-devel` group)
- **Disk:** ~500 MB free (target/ dir + world model storage)

## 1. Clone and Build

```bash
git clone https://github.com/ai-os/ai-os.git
cd ai-os
cargo build --release -p desktop-companion
```

Build takes 3–8 minutes depending on hardware.

## 2. Configure LLM Provider

The companion reads settings from `~/.config/ai-os-companion/settings.json`.
On first run, a default config is created automatically. Edit it before starting:

```bash
mkdir -p ~/.config/ai-os-companion
cat > ~/.config/ai-os-companion/settings.json << 'EOF'
{
  "llm_provider": "openai",
  "llm_api_key": "sk-your-key-here",
  "llm_model": "gpt-4o",
  "nvapi_token": "",
  "observation": {
    "interval_secs": 60,
    "observation_window": 300,
    "linux_processes": true,
    "linux_system": true
  },
  "notifications": {
    "enabled": true,
    "sound": true,
    "quiet_hours_start": "",
    "quiet_hours_end": ""
  },
  "attention_sensitivity": 0.7,
  "reflection_frequency_secs": 300,
  "autostart": false,
  "developer_mode": false,
  "debug_logging": false,
  "wm_storage_path": "~/.local/share/ai-os-companion/wm/world_model.json",
  "ui_port": 9876,
  "privacy": {
    "encrypt_storage": false,
    "local_only": true,
    "backup_count": 3
  }
}
EOF
```

### LLM Provider Configuration

Edit `~/.config/ai-os-companion/settings.json` and set `llm_provider` to your provider:

| Provider | `llm_provider` | Extra settings |
|---|---|---|
| OpenAI | `"openai"` | `llm_api_key`, `llm_model` |
| Anthropic | `"anthropic"` | `llm_api_key`, `llm_model` |
| Ollama (local) | `"ollama"` | Model must be pulled (`ollama list`) |
| NVIDIA NVAPI | `"nvapi"` | `nvapi_token` (see below) |
| Disabled | `""` or omitted | Companion observes but cannot reason |

#### NVAPI (Nvidia GPU Inference)

To use NVIDIA's NVAPI (NIM endpoints), add to `settings.json`:

```json
{
  "llm_provider": "nvapi",
  "nvapi_token": "nvapi-your-token-here",
  "llm_model": "nim://meta/llama3-70b-instruct"
}
```

The token is stored locally and used as a Bearer token against `https://api.nvidia.com/v1`.
Never share your NVAPI token publicly. The token is never written to log files.

## 3. Start the Companion

```bash
# One-time: install binary (optional)
sudo cp target/release/desktop-companion /usr/local/bin/

# Run from source (development)
./target/release/desktop-companion
```

Or if installed system-wide:

```bash
desktop-companion
```

You should see:

```
[INFO] AI-OS Companion v0.3.0 starting...
[INFO] World Model: /home/arch/.local/share/ai-os-companion/wm/world_model.json
[INFO] Web UI: http://localhost:9876
[INFO] Companion is running. Press Ctrl+C to stop.
```

## 4. Open the Desktop UI

Open a browser and navigate to:

```
http://localhost:9876
```

You should see the Companion Dashboard with tabs:

- **Chat** — send messages and receive responses
- **Status** — current cycle, entity count, last decision
- **World Model** — entities and relationships
- **Settings** — modify configuration (applies on restart)
- **Dashboard** — developer view (requires `developer_mode: true`)

## 5. Begin Chatting

Type a message in the Chat tab and press Enter. The companion will:

1. Observe the current environment (processes, system state)
2. Reflect on the observation
3. Decide how to respond
4. Execute any actions
5. Persist everything to the World Model

## 6. Stop the Companion

Press `Ctrl+C` in the terminal. The world model is saved before shutdown.

---

## Troubleshooting

### "Permission denied" on settings file

```bash
chmod 600 ~/.config/ai-os-companion/settings.json
```

### Web UI not loading

Verify the port is free and matches your settings:

```bash
curl http://localhost:9876/api/status
```

If using a different port, check your `settings.json` → `ui_port`.

### "No such file or directory" for world model

The WM storage directory is created automatically. If it fails:

```bash
mkdir -p ~/.local/share/ai-os-companion/wm
```

### Companion crashes on startup

Check the log file (if configured) or run with debug logging:

```bash
RUST_LOG=debug desktop-companion 2>&1 | tail -30
```

### LLM responses feel slow

- Check `llm_provider` and `llm_model` in `settings.json`
- For local providers (Ollama), verify the model is pulled: `ollama list`
- Network latency to the API endpoint

### Notifications not appearing

```bash
# Test notify-send manually
notify-send "test" "hello"
```

If `notify-send` is missing: `sudo pacman -S libnotify`

---

## Environment Variables

| Variable | Purpose | Example |
|---|---|---|
| `RUST_LOG` | Log level | `info`, `debug`, `warn`, `error` |
| `HOME` | User home (default config path) | `/home/arch` |
| `AIOS_CONFIG` | Override settings path | `/etc/ai-os/settings.json` |

---

## File Locations

| File | Purpose |
|---|---|
| `~/.config/ai-os-companion/settings.json` | Settings |
| `~/.config/ai-os-companion/.encryption_key` | Encryption key (if enabled) |
| `~/.local/share/ai-os-companion/wm/world_model.json` | World Model persistence |
| `~/.local/share/ai-os-companion/backups/` | Backup copies |
| `~/.config/autostart/ai-os-companion.desktop` | Autostart entry |
| `/var/log/ai-os/` | Log files (if configured) |
