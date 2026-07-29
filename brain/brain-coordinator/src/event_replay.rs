use crate::goal_scheduler::GoalScheduler;
use brain_core::ids::GoalId;
use memory_core::Timestamp;
use std::sync::Arc;
use tokio::sync::RwLock;

/// A recorded event for replay.
#[derive(Debug, Clone)]
pub struct RecordedEvent {
    pub event_type: String,
    pub payload: Option<String>,
    pub goal_id: Option<GoalId>,
    pub recorded_at: Timestamp,
}

/// Event journal for persistence and replay.
///
/// Records important events so that on restart, the runtime can
/// replay missed events and restore the World Model.
#[derive(Debug)]
pub struct EventJournal {
    events: RwLock<Vec<RecordedEvent>>,
    max_entries: usize,
}

impl EventJournal {
    pub fn new() -> Self {
        Self {
            events: RwLock::new(Vec::new()),
            max_entries: 1000,
        }
    }

    pub fn with_max_entries(mut self, max: usize) -> Self {
        self.max_entries = max;
        self
    }

    /// Record an event.
    pub async fn record(
        &self,
        event_type: impl Into<String>,
        payload: Option<String>,
        goal_id: Option<GoalId>,
    ) {
        let mut events = self.events.write().await;
        if events.len() >= self.max_entries {
            events.remove(0);
        }
        events.push(RecordedEvent {
            event_type: event_type.into(),
            payload,
            goal_id,
            recorded_at: Timestamp::now(),
        });
    }

    /// Return all recorded events.
    pub async fn all(&self) -> Vec<RecordedEvent> {
        self.events.read().await.clone()
    }

    /// Clear the journal.
    pub async fn clear(&self) {
        self.events.write().await.clear();
    }

    /// Number of recorded events.
    pub async fn len(&self) -> usize {
        self.events.read().await.len()
    }

    pub async fn is_empty(&self) -> bool {
        self.len().await == 0
    }
}

impl Default for EventJournal {
    fn default() -> Self {
        Self::new()
    }
}

/// Manages replay of events on runtime restart.
#[derive(Debug)]
pub struct EventReplayManager {
    journal: Arc<EventJournal>,
}

impl EventReplayManager {
    pub fn new(journal: Arc<EventJournal>) -> Self {
        Self { journal }
    }

    /// Replay all recorded events through the goal scheduler.
    ///
    /// Returns the list of goal IDs that were woken by replayed events.
    pub async fn replay(&self, scheduler: &GoalScheduler) -> Vec<GoalId> {
        let events = self.journal.all().await;
        let mut woken = Vec::new();

        for event in &events {
            let matched = scheduler.match_event(&event.event_type).await;
            for gid in matched {
                if !woken.contains(&gid) {
                    woken.push(gid);
                }
            }
            // Also add goal-scoped events
            if let Some(gid) = event.goal_id {
                if !woken.contains(&gid) {
                    woken.push(gid);
                }
            }
        }

        woken
    }

    /// Replay and then clear the journal.
    pub async fn replay_and_clear(&self, scheduler: &GoalScheduler) -> Vec<GoalId> {
        let woken = self.replay(scheduler).await;
        self.journal.clear().await;
        woken
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::goal_scheduler::{EventSubscription, GoalScheduler};
    use brain_core::ids::GoalId;
    use brain_core::types::GoalPriority;

    #[tokio::test]
    async fn record_and_replay() {
        let journal = Arc::new(EventJournal::new());
        let scheduler = GoalScheduler::new();

        let gid = GoalId::new();
        scheduler
            .register(
                gid,
                GoalPriority::Normal,
                vec![EventSubscription::exact("test.event")],
            )
            .await;

        journal.record("test.event", None, None).await;
        assert_eq!(journal.len().await, 1);

        let manager = EventReplayManager::new(journal.clone());
        let woken = manager.replay(&scheduler).await;
        assert_eq!(woken, vec![gid]);
    }

    #[tokio::test]
    async fn replay_and_clear_empties_journal() {
        let journal = Arc::new(EventJournal::new());
        let scheduler = GoalScheduler::new();

        journal.record("test.event", None, None).await;
        journal.record("other.event", None, None).await;

        let manager = EventReplayManager::new(journal.clone());
        let _woken = manager.replay_and_clear(&scheduler).await;
        assert!(journal.is_empty().await);
    }

    #[tokio::test]
    async fn empty_journal_replay() {
        let journal = Arc::new(EventJournal::new());
        let scheduler = GoalScheduler::new();
        let manager = EventReplayManager::new(journal);
        let woken = manager.replay(&scheduler).await;
        assert!(woken.is_empty());
    }

    #[tokio::test]
    async fn journal_max_entries() {
        let journal = Arc::new(EventJournal::new().with_max_entries(3));
        for i in 0..5 {
            journal.record(format!("event.{i}"), None, None).await;
        }
        assert_eq!(journal.len().await, 3);
        let all = journal.all().await;
        // Oldest entries should be evicted
        assert_eq!(all[0].event_type, "event.2");
        assert_eq!(all[2].event_type, "event.4");
    }

    #[tokio::test]
    async fn goal_scoped_event_replay() {
        let journal = Arc::new(EventJournal::new());
        let scheduler = GoalScheduler::new();

        let gid = GoalId::new();
        scheduler.register(gid, GoalPriority::Normal, vec![]).await;

        journal.record("custom.event", None, Some(gid)).await;

        let manager = EventReplayManager::new(journal);
        let woken = manager.replay(&scheduler).await;
        assert_eq!(woken, vec![gid]);
    }
}
