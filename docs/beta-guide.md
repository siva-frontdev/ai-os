# AI-OS Beta Guide

Welcome to the AI-OS Living Beta. This guide explains how to use AI-OS daily, report issues, export logs, reproduce problems, and contribute to behavior improvement.

---

## Daily Usage

### Morning Routine

1. **Start the companion**
   ```bash
   desktop-companion
   ```
2. **Check the Dashboard** — open `http://localhost:9876`
3. **Review the Status tab** — look at yesterday's reflections and today's observations
4. **Begin chatting** — the companion has context from your world model

The companion automatically observes your environment (processes, system state, time-based patterns) and reflects on observations periodically.

### During the Day

- Chat normally via the web UI — the companion remembers context between exchanges
- The companion may send notifications if it detects important patterns
- All interactions persist to the World Model automatically

### End of Day

- **Stop the companion** with `Ctrl+C` — this triggers graceful shutdown and world model persistence
- Alternatively, the companion continues running in the background as a daemon

### Shutting Down

```bash
# If running in foreground
# Press Ctrl+C

# If running as a service
sudo systemctl stop ai-os-companion
```

On shutdown, the companion:
1. Saves the current World Model to disk
2. Closes the web UI listener
3. Stops the observation loop
4. Exits cleanly

---

## Reporting Behavior Issues

Every behavior issue should use the [Behavior Issue Template](behavior-issue-template.md).

### Before Reporting

1. **Reproduce the issue** — confirm it's consistent, not a one-off
2. **Export logs** — see below
3. **Check if it's known** — look at existing issues

### What Makes a Good Report

A good report includes:

- **Situation** — what you were doing when the behavior occurred
- **Expected behavior** — what you expected the companion to do
- **Actual behavior** — what it actually did
- **Observed reasoning** — what the companion said/decided (screenshot or transcript)
- **Responsible subsystem** — which module is likely at fault
- **Root cause** — your best hypothesis
- **Minimal fix** — if you know the fix, contribute it
- **Regression scenario** — steps to verify the issue is fixed

---

## Exporting Logs

### Web UI Export

1. Open `http://localhost:9876`
2. Navigate to **Dashboard** (requires `developer_mode: true`)
3. Click **Export Logs** button — downloads a JSON bundle containing observations, decisions, reflections, and privacy events for the current session

### Manual Export

```bash
# Export recent debug output
journalctl -u ai-os-companion --since "1 hour ago" > ai-os-export.log

# Export to a self-contained archive
tar -czf ai-os-export-$(date +%Y%m%d-%H%M%S).tar.gz \
  ~/.config/ai-os-companion/ \
  ~/.local/share/ai-os-companion/ \
  /var/log/ai-os/ 2>/dev/null
```

### What's Included in an Export

| Data | Location | Included |
|---|---|---|
| Settings | `~/.config/ai-os-companion/settings.json` | Yes |
| World Model | `~/.local/share/ai-os-companion/wm/` | Yes |
| Backups | `~/.local/share/ai-os-companion/backups/` | Yes |
| Session logs | Terminal output / journalctl | Yes |
| Privacy events | API `/api/privacy/events` | Yes |
| Audit log | API `/api/audit` | Yes |

### Privacy Considerations

- Export files may contain the World Model which includes observations about your system
- If encryption is enabled, exports are already encrypted
- Never share exports publicly without sanitizing sensitive information
- Use `AIOS_EXPORT_PRIVATE=false` to exclude sensitive data from exports

---

## Reproducing Issues

1. **Identify the trigger** — what action led to the issue?
2. **Note the state** — what was the World Model like at the time?
3. **Recreate the trigger** — repeat the same action
4. **Observe the behavior** — does the same issue occur?
5. **Export logs** — capture the debug output during reproduction
6. **File an issue** — use the Behavior Issue Template

### Minimal Reproduction

For the companion's cognitive loop:

```bash
# 1. Start fresh
rm -rf ~/.local/share/ai-os-companion/wm/*

# 2. Start companion
desktop-companion

# 3. Wait for one full observation cycle (default: 60 seconds)
# 4. Chat about a specific topic
# 5. Check if the companion's response matches expectations
# 6. Press Ctrl+C to stop
```

---

## Improving Behavior

### Feedback Loop

1. **Use the companion daily** — every interaction trains the World Model
2. **Correct the companion when it's wrong** — explicit corrections improve future decisions
3. **Provide context** — tell the companion about your goals, preferences, and constraints
4. **Review reflections** — the Companion Dashboard shows what the companion noticed and whether it was accurate
5. **Adjust sensitivity** — raise `attention_sensitivity` in settings if the companion is too passive; lower it if too noisy

### Settings Tuning

| Setting | Increase For | Decrease For |
|---|---|---|
| `attention_sensitivity` | More proactive companion | Fewer interruptions |
| `reflection_frequency_secs` | Faster self-improvement | Less overhead |
| `observation.interval_secs` | More frequent observations | Lower resource usage |
| `notifications.enabled` | More alerting | Quieter experience |

### Contributing Fixes

When you identify a behavior issue with a clear fix:

1. Create an issue using the Behavior Issue Template
2. Include a minimal reproduction case
3. Propose a fix (PR welcome)
4. Tests should cover the fix

---

## Beta Etiquette

- **Expect instability** — AI-OS is a Living Beta; behavior may change between sessions
- **Save your data** — World Model persistence is reliable but backups are recommended
- **Report regressions immediately** — behavioral regressions are the highest priority
- **Don't rely on AI-OS for critical decisions** — the companion is a research tool, not a decision-maker for safety-critical tasks
- **Privacy first** — understand what data the companion collects and where it's stored
