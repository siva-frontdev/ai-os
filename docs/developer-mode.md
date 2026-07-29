# Developer Mode

Developer mode provides deep inspection into AI-OS internals. Enable it in `settings.json`:

```json
{ "developer_mode": true }
```

The Developer Dashboard tab appears in the web UI at `http://localhost:9876`.

---

## Inspecting the World Model

The World Model is a knowledge graph of entities and relationships.

### Via the Web UI

1. Open `http://localhost:9876`
2. Navigate to the **World Model** tab
3. Filter by entity type (person, project, concept, event, etc.)
4. Click any entity to inspect its properties, importance, and relationships
5. Search by name to find entities or relationships

### Via API

```bash
# List all entities (last 50 by importance)
curl http://localhost:9876/api/entities?limit=50

# List all relationships
curl http://localhost:9876/api/relationships

# Search entities by name
curl "http://localhost:9876/api/entities?search=Alice"

# Get entity by ID
curl http://localhost:9876/api/entities/<entity-id>

# Get relationships for an entity
curl http://localhost:9876/api/entities/<entity-id>/relationships
```

### Via Code (Rust)

```rust
use brain_coordinator::companion_host::CompanionHost;

let entity_count = host.store.entity_count().await;
let entities = host.store.search_entities_by_name("Alice").await;
let relationships = host.store.get_relationships_for_entity(&entity_id).await;
let all_entities = host.store.all_entities().await;
```

### What You'll See

Entities have these properties:
- `entity_type` — person, project, concept, event, task, etc.
- `name` — display name
- `importance` — 0.0 to 1.0
- `updated_at` — last modification timestamp
- `metadata` — key-value pairs

Relationships have:
- `source_entity` — from entity ID
- `target_entity` — to entity ID
- `relationship_type` — `member_of`, `depends_on`, `communicated_with`, etc.
- `importance` — 0.0 to 1.0

---

## Inspecting Reflection

Reflection is the companion's self-examination process that runs every N cycles.

### Via the Web UI

Navigate to the **Reflections** tab. Each entry shows:
- **Timestamp** — when the reflection ran
- **Category** — insight, pattern, concern, decision, memory
- **Summary** — what the companion noticed
- **Confidence** — how certain the companion was
- **Action taken** — what changed as a result

### Via API

```bash
# Get all reflections (last 20)
curl http://localhost:9876/api/reflections

# Get reflections by category
curl "http://localhost:9876/api/reflections?category=insight"

# Get a specific reflection
curl http://localhost:9876/api/reflections/<id>
```

### What a Reflection Entry Contains

```json
{
  "id": "ref_abc123",
  "timestamp": "2026-07-28T10:30:00Z",
  "category": "insight",
  "summary": "Alice contacts the team every morning at 9am",
  "confidence": 0.85,
  "action": "Created recurring_event entity for Alice morning check-in"
}
```

---

## Inspecting Attention

Attention determines what the companion focuses on during each cycle.

### Via the Web UI

The **Status** tab shows the current attention signal. The **Dashboard** tab (developer mode) shows the full attention history with outcome labels.

### Via API

```bash
# Current attention decision
curl http://localhost:9876/api/attention/current

# Attention history
curl http://localhost:9876/api/attention/history

# Attention stats
curl http://localhost:9876/api/attention/stats
```

### Attention Outcomes

| Outcome | Meaning |
|---|---|
| `reflect` | Attention detected something meaningful — reflecting now |
| `observe` | Continue observing — nothing notable yet |
| `communicate` | Attention triggered a user communication |
| `idle` | No attention signal — low activity period |

---

## Inspecting Decisions

Every decision the companion makes is recorded for inspection.

### Via the Web UI

The **Status** tab shows the most recent decision. The **Dashboard** tab shows the decision log.

### Via API

```bash
# Last decision
curl http://localhost:9876/api/decisions/latest

# Decision log
curl http://localhost:9876/api/decisions?limit=20

# Decisions by type
curl "http://localhost:9876/api/decisions?type=Communicate"

# Decision reasoning chain
curl http://localhost:9876/api/decisions/<id>/reasoning
```

### What a Decision Contains

```json
{
  "id": "dec_xyz789",
  "timestamp": "2026-07-28T10:31:05Z",
  "decision_type": "Communicate",
  "input": "User typed: 'What did I work on this morning?'",
  "reasoning": [
    "Observe: retrieved morning observations from World Model",
    "Reflect: identified work context from entity relations",
    "Decide: choose Communicate action to share findings"
  ],
  "output": "You worked on the AI-OS platform build this morning.",
  "confidence": 0.9,
  "duration_ms": 450
}
```

---

## Inspecting Communication

Communications are the companion's outbound messages (notifications, chat responses).

### Via the Web UI

The **Chat** tab shows full conversation history. Each message shows the companion's reasoning.

### Via API

```bash
# Full conversation
curl http://localhost:9876/api/chat/history

# Last N messages
curl "http://localhost:9876/api/chat/history?limit=10"

# Communication audit trail (all notifications sent)
curl http://localhost:9876/api/audit/communications
```

---

## Inspecting Privacy Logs

The Privacy module logs all encryption, backup, and access events.

### Via API

```bash
# All privacy events
curl http://localhost:9876/api/privacy/events

# Encryption status
curl http://localhost:9876/api/privacy/status

# Backup history
curl http://localhost:9876/api/privacy/backups

# Access log (who accessed what and when)
curl http://localhost:9876/api/privacy/access-log
```

### Privacy Events

| Event | Description |
|---|---|
| `encryption_enabled` | Encryption was turned on |
| `encryption_disabled` | Encryption was turned off |
| `key_generated` | New encryption key created |
| `key_accessed` | Key was read for encryption/decryption |
| `backup_created` | A backup was created |
| `backup_restored` | A backup was restored |
| `backup_rotated` | Old backups were pruned |
| `key_file_permissions_set` | Key file permissions were tightened |

### Key File Permissions

When encryption is enabled, the key file is created with mode `0o400` (owner read-only):
- No group or world access
- Stored in `~/.config/ai-os-companion/.encryption_key`
- Never logged (the key content itself is never written to log files)

---

## Developer Mode Settings

| Setting | Default | Effect |
|---|---|---|
| `developer_mode` | `false` | Shows Developer Dashboard tab |
| `debug_logging` | `false` | Enables trace-level logging to stdout |

### Debug Logging Example

```bash
# Run with full debug output
RUST_LOG=debug desktop-companion 2>&1 | tee debug.log

# Filter to just the cognitive loop
RUST_LOG=brain_coordinator=debug,ai_os_core=warn desktop-companion 2>&1 | grep "CYCLE\|REFLECT\|DECIDE"
```
