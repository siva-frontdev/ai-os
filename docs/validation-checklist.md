# Real-World Validation Checklist

This checklist validates AI-OS for daily use. Run each scenario on a fresh install before relying on the companion.

---

## 1. Morning Startup

**Steps:**
1. Boot the machine
2. Start `desktop-companion`
3. Wait for the Startup message: `Companion is running. Press Ctrl+C to stop.`
4. Open `http://localhost:9876` in a browser
5. Verify the Dashboard loads
6. Check that the World Model loads from persistence

**Expected:**
- Companion starts within 10 seconds
- Web UI is accessible within 5 seconds of startup
- World Model loads without errors (check companion log)
- No duplicate entities from previous session

**Verify:**
```bash
curl -s http://localhost:9876/api/status | jq '.running'
# Expected: true
curl -s http://localhost:9876/api/entities | jq '.count'
# Should return a non-negative integer
```

---

## 2. Conversation Continuity

**Steps:**
1. Start the companion fresh
2. Chat about a topic (e.g., "What did I work on this morning?")
3. Note the companion's response
4. Chat again in a new session (stop and restart companion)
5. Ask the same question or a follow-up
6. Verify the companion remembers

**Expected:**
- The companion recalls context from previous sessions
- Follow-up questions reference earlier conversation
- World Model entities grow as the companion learns new information

---

## 3. Notification Quality

**Steps:**
1. Enable notifications in settings (`notifications.enabled: true`)
2. Configure a meaningful attention sensitivity (`attention_sensitivity: 0.5`)
3. Interact with the companion for 10+ minutes
4. Check for desktop notifications appearing
5. Verify notifications are relevant, not spam

**Expected:**
- Notifications appear for important events only
- No notification spam (more than 1 notification per 5-minute window)
- Notifications carry meaningful content (not "hello" or empty)
- Quiet hours are respected if configured

---

## 4. Observation Quality

**Steps:**
1. Open several applications (browser, terminal, editor)
2. Leave the companion running for one observation cycle (default: 60s)
3. Check the Dashboard Observation tab
4. Verify the observed system state is accurate

**Expected:**
- Running processes are correctly reported
- System load is accurate (matches `top` or `htop`)
- Time-based observations are timestamped correctly
- No false positives or missed events

---

## 5. Trust

**Steps:**
1. Share a sensitive piece of information with the companion in chat
2. Verify it is persisted with encryption (if encryption is enabled)
3. Stop and restart the companion
4. Verify the information is still accessible (if appropriate)
5. Verify it is NOT accessible to other users on the system

**Expected:**
- Sensitive data is stored with appropriate permissions (600 or 400)
- Encryption key file is not world-readable
- The companion does not echo sensitive data in notifications or logs
- Privacy config (`local_only: true`) is respected — no network data leakage

---

## 6. Silence

**Steps:**
1. Do not interact with the companion for several hours
2. Monitor the companion's CPU usage
3. Check that the companion does not generate spam

**Expected:**
- CPU usage stays low when idle (near 0%)
- No duplicate notifications
- No unnecessary observations or reflections
- The observation interval setting is respected (no premature observation)

---

## 7. Judgment

**Steps:**
1. Ask the companion a complex question requiring multiple steps
2. Observe how it reasons through the problem
3. Check the Decisions tab for reasoning trail
4. Verify the final answer is correct

**Expected:**
- The companion shows its reasoning chain (not just a final answer)
- Complex questions get multi-step reasoning
- Confidence scores are reasonable (not always 100%)
- The companion says "I don't know" when it cannot answer

---

## 8. Persistence

**Steps:**
1. Have the companion build a rich World Model over 1+ day of use
2. Stop the companion (`Ctrl+C`)
3. Restart the companion
4. Verify all entities and relationships survived shutdown
5. Add more data, stop again, restart again
6. Confirm continuity across multiple stop/start cycles

**Expected:**
- World Model is restored from `~/.local/share/ai-os-companion/wm/world_model.json` on startup
- No data loss between sessions
- Entity counts match pre-shutdown values
- Backups are created according to `backup_count` config

---

## 9. Restart Recovery

**Steps:**
1. Kill the companion mid-cycle (`SIGKILL` or `kill -9`):
   ```bash
   pkill -9 desktop-companion
   ```
2. Restart the companion
3. Verify it recovers cleanly
4. Check that the World Model is not corrupted
5. Check that no duplicate data was created

**Expected:**
- Companion starts without errors after a forced kill
- World Model is not corrupted (all JSON loads correctly)
- No duplicate entities from the interrupted cycle
- The observation loop resumes from where it left off

---

## Running the Full Checklist

```bash
# Run all checks in sequence
./scripts/validate.sh

# Run a specific check (example: startup)
./scripts/validate.sh startup
```

---

## Results Template

After completing each scenario, record:

| Scenario | Result | Notes |
|---|---|---|
| 1. Morning Startup | ✅ / ❌ | |
| 2. Conversation Continuity | ✅ / ❌ | |
| 3. Notification Quality | ✅ / ❌ | |
| 4. Observation Quality | ✅ / ❌ | |
| 5. Trust | ✅ / ❌ | |
| 6. Silence | ✅ / ❌ | |
| 7. Judgment | ✅ / ❌ | |
| 8. Persistence | ✅ / ❌ | |
| 9. Restart Recovery | ✅ / ❌ | |
