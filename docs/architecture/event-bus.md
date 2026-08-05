# Event Bus Architecture

## Overview

The Event Bus is the universal communication backbone of AI-OS. All inter-component communication flows through typed, traceable events. No direct references between components.

## Core Event Bus (`core/src/events/mod.rs`)

### Trait Definition
```rust
pub trait EventBus: Debug + Send + Sync {
    async fn publish(&self, event: Box<dyn Event>) -> Result<(), CoreError>;
    async fn subscribe(&self, event_type: &'static str) -> Result<EventStream, CoreError>;
}
```

### Event Trait
```rust
pub trait Event: Debug + Send + Sync {
    fn event_type(&self) -> &'static str;
}
```

### InMemoryEventBus
- Async, non-blocking dispatch
- Type-erased handler routing via `TypeId`
- Publishers never wait for subscribers
- Events carry trace context for distributed tracing

## Brain Event Driver (`brain/brain-coordinator/src/event_driver.rs`)

### RuntimeEvent
```rust
pub struct RuntimeEvent {
    pub event_type: String,
    pub payload: Option<String>,
    pub goal_id: Option<GoalId>,
}
```

### EventDriver Loop
```
receive event
    │
    ▼
record in EventJournal (replay)
    │
    ▼
update World Model
    │
    ▼
match against GoalScheduler subscriptions
    │
    ▼
run cognitive_tick_for() for each affected goal
    │
    ▼
check plan health / repair
    │
    ▼
wait for next event
```

Heartbeat runs maintenance every 10s (deadlock detection, goal expiration, cache cleanup).

## Event Replay (`brain/brain-coordinator/src/event_replay.rs`)

### EventJournal
```rust
pub struct EventJournal {
    events: Vec<RecordedEvent>,
    max_events: usize,
}
```

### EventReplayManager
- Records all events with timestamps
- Replays from checkpoint for recovery
- Filters by event type / goal / time range

### RecordedEvent
```rust
pub struct RecordedEvent {
    pub timestamp: Timestamp,
    pub event_type: String,
    pub payload: Option<String>,
    pub goal_id: Option<GoalId>,
}
```

## Standard Event Types

| Event Type | Source | Payload | Description |
|---|---|---|---|
| `runtime.maintenance` | Heartbeat | — | Periodic maintenance tick |
| `brain.goal.created` | GoalManager | GoalRecord | New goal created |
| `brain.goal.activated` | GoalManager | GoalId | Goal moved to Active |
| `brain.goal.completed` | GoalManager | GoalRecord | Goal finished successfully |
| `brain.goal.failed` | GoalManager | GoalRecord | Goal failed |
| `brain.goal.paused` | GoalManager | GoalRecord | Goal paused |
| `brain.goal.retried` | GoalManager | GoalRecord | Goal retried |
| `brain.goal.reprioritized` | GoalManager | GoalRecord | Priority changed |
| `brain.plan.created` | Orchestrator | ExecutablePlan | New plan generated |
| `brain.reflection.completed` | Reflector | Reflection | Reflection cycle done |
| `brain.learning.feedback` | LearningEngine | LearningFeedback | New patterns learned |
| `runtime.observation` | RuntimeManager | Observation | Plugin observation emitted |
| `runtime.action.dispatched` | RuntimeManager | Action | Capability dispatched |
| `runtime.action.completed` | RuntimeManager | ActionResult | Action finished |

## Integration Points

### Brain Orchestrator
```rust
fn emit_event(&self, event_type: &str, goal_id: Option<GoalId>) {
    if let Some(ref tx) = self.event_tx {
        let _ = tx.send((event_type.to_string(), goal_id));
    }
}
```

### EventDriver Bridge
```rust
pub(crate) struct EventBridge {
    tx: mpsc::UnboundedSender<RuntimeEvent>,
    event_type: &'static str,
}

impl EventBridge {
    pub async fn handle_event(&self, payload: Option<String>) {
        let event = RuntimeEvent::new(self.event_type)
            .with_payload(payload.unwrap_or_default());
        let _ = self.tx.send(event);
    }
}
```

### Core EventBus Subscription
```rust
// In EventDriver::new(), subscribe to core EventBus:
event_bus.subscribe("FileChanged")?.await
    .map(|e| bridge.handle_event(Some(e.payload)))
```

## Testing

- `brain/brain-coordinator/src/event_replay.rs` — unit tests for journal/replay
- `brain/brain-coordinator/src/event_driver.rs` — unit tests for event matching

## Design Principles

1. **No direct coupling** — Components only know event types, not each other
2. **Typed events** — Every event has a statically known `event_type()` string
3. **Trace context** — Events carry `goal_id` for distributed tracing
4. **Replayability** — All events recorded for recovery/debugging
5. **Async, non-blocking** — Publish returns immediately