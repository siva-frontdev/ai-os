use brain_core::ids::GoalId;
use brain_core::types::GoalPriority;
use memory_core::Timestamp;
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::RwLock;

fn timestamp_add_duration(ts: Timestamp, dur: Duration) -> Timestamp {
    Timestamp::from_nanos(ts.as_nanos().saturating_add(dur.as_nanos() as i64))
}

/// A subscription ties a goal to the event types that should wake it.
#[derive(Debug, Clone)]
pub struct EventSubscription {
    /// Exact or prefix event type string (e.g. "brain.goal.completed", "runtime.task.").
    pub pattern: String,
    /// If true, `pattern` is treated as a prefix match.
    pub prefix_match: bool,
}

impl EventSubscription {
    pub fn exact(pattern: impl Into<String>) -> Self {
        Self {
            pattern: pattern.into(),
            prefix_match: false,
        }
    }

    pub fn prefix(pattern: impl Into<String>) -> Self {
        Self {
            pattern: pattern.into(),
            prefix_match: true,
        }
    }

    pub fn matches(&self, event_type: &str) -> bool {
        if self.prefix_match {
            event_type.starts_with(&self.pattern)
        } else {
            event_type == self.pattern
        }
    }
}

/// Scheduling metadata for a single goal.
#[derive(Debug, Clone)]
pub struct GoalSchedule {
    pub goal_id: GoalId,
    pub priority: GoalPriority,
    pub subscriptions: Vec<EventSubscription>,
    pub review_interval_ms: u64,
    pub next_review: Timestamp,
    pub last_event_wake: Option<Timestamp>,
}

/// Adaptive goal scheduler driven by event subscriptions and review timers.
///
/// Goals are woken up when:
/// - A matching event arrives via `match_event`
/// - The review timer expires (checked via `due_goals`)
///
/// The review interval adapts based on priority and event frequency.
#[derive(Debug)]
pub struct GoalScheduler {
    schedules: RwLock<HashMap<GoalId, GoalSchedule>>,
    event_count: RwLock<HashMap<String, u64>>,
}

impl GoalScheduler {
    pub fn new() -> Self {
        Self {
            schedules: RwLock::new(HashMap::new()),
            event_count: RwLock::new(HashMap::new()),
        }
    }

    pub async fn register(
        &self,
        goal_id: GoalId,
        priority: GoalPriority,
        subscriptions: Vec<EventSubscription>,
    ) {
        let interval = Self::default_interval(&priority);
        let mut schedules = self.schedules.write().await;
        schedules.insert(
            goal_id,
            GoalSchedule {
                goal_id,
                priority,
                subscriptions,
                review_interval_ms: interval,
                next_review: Timestamp::now(),
                last_event_wake: None,
            },
        );
    }

    pub async fn unregister(&self, goal_id: &GoalId) {
        let mut schedules = self.schedules.write().await;
        schedules.remove(goal_id);
    }

    /// Update priority – also adjusts the review interval.
    pub async fn update_priority(&self, goal_id: &GoalId, new_priority: GoalPriority) {
        let mut schedules = self.schedules.write().await;
        if let Some(s) = schedules.get_mut(goal_id) {
            s.priority = new_priority;
            s.review_interval_ms = Self::default_interval(&new_priority);
        }
    }

    /// Add a subscription to an existing goal.
    pub async fn add_subscription(&self, goal_id: &GoalId, sub: EventSubscription) {
        let mut schedules = self.schedules.write().await;
        if let Some(s) = schedules.get_mut(goal_id) {
            s.subscriptions.push(sub);
        }
    }

    /// Find all goals whose subscriptions match `event_type`.
    /// Also records the event for adaptive interval tuning.
    pub async fn match_event(&self, event_type: &str) -> Vec<GoalId> {
        // Record event for adaptive tuning
        {
            let mut counts = self.event_count.write().await;
            *counts.entry(event_type.to_string()).or_insert(0) += 1;
        }

        let schedules = self.schedules.read().await;
        let mut matched = Vec::new();
        for schedule in schedules.values() {
            for sub in &schedule.subscriptions {
                if sub.matches(event_type) {
                    matched.push(schedule.goal_id);
                    break;
                }
            }
        }
        matched
    }

    /// Return goals whose review timer has expired.
    pub async fn due_goals(&self) -> Vec<GoalId> {
        let now = Timestamp::now();
        let schedules = self.schedules.read().await;
        let mut due = Vec::new();
        for schedule in schedules.values() {
            if schedule.next_review <= now {
                due.push(schedule.goal_id);
            }
        }
        due
    }

    /// Reset the review timer for a goal (called after processing).
    pub async fn reset_timer(&self, goal_id: &GoalId) {
        let mut schedules = self.schedules.write().await;
        if let Some(s) = schedules.get_mut(goal_id) {
            let adaptive = self.adaptive_interval(s).await;
            s.next_review =
                timestamp_add_duration(Timestamp::now(), Duration::from_millis(adaptive));
            s.last_event_wake = Some(Timestamp::now());
        }
    }

    /// Adaptive interval: high-priority goals and goals with frequent
    /// events get shorter intervals.
    async fn adaptive_interval(&self, schedule: &GoalSchedule) -> u64 {
        let base = schedule.review_interval_ms;
        let event_freq = {
            let counts = self.event_count.read().await;
            let total: u64 = counts.values().sum();
            total
        };
        // If many events are flowing, shorten intervals for high-priority goals
        match schedule.priority {
            GoalPriority::Critical if event_freq > 50 => base / 4,
            GoalPriority::High if event_freq > 50 => base / 2,
            GoalPriority::Critical => base / 2,
            _ => base,
        }
        .max(100) // floor at 100ms
    }

    fn default_interval(priority: &GoalPriority) -> u64 {
        match priority {
            GoalPriority::Critical => 500,
            GoalPriority::High => 2000,
            GoalPriority::Normal => 10_000,
            GoalPriority::Low => 60_000,
        }
    }

    /// Total tracked goals.
    pub async fn len(&self) -> usize {
        self.schedules.read().await.len()
    }

    pub async fn is_empty(&self) -> bool {
        self.len().await == 0
    }
}

impl Default for GoalScheduler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use brain_core::ids::GoalId;

    #[tokio::test]
    async fn register_and_unregister() {
        let scheduler = GoalScheduler::new();
        let gid = GoalId::new();
        assert!(scheduler.is_empty().await);
        scheduler.register(gid, GoalPriority::Normal, vec![]).await;
        assert_eq!(scheduler.len().await, 1);
        scheduler.unregister(&gid).await;
        assert!(scheduler.is_empty().await);
    }

    #[tokio::test]
    async fn exact_match_wakes_goal() {
        let scheduler = GoalScheduler::new();
        let gid = GoalId::new();
        scheduler
            .register(
                gid,
                GoalPriority::Normal,
                vec![EventSubscription::exact("brain.goal.completed")],
            )
            .await;
        let matched = scheduler.match_event("brain.goal.completed").await;
        assert_eq!(matched, vec![gid]);
    }

    #[tokio::test]
    async fn prefix_match_wakes_goal() {
        let scheduler = GoalScheduler::new();
        let gid = GoalId::new();
        scheduler
            .register(
                gid,
                GoalPriority::Normal,
                vec![EventSubscription::prefix("runtime.task.")],
            )
            .await;
        let matched = scheduler.match_event("runtime.task.completed").await;
        assert_eq!(matched, vec![gid]);
    }

    #[tokio::test]
    async fn no_match_returns_empty() {
        let scheduler = GoalScheduler::new();
        let gid = GoalId::new();
        scheduler
            .register(
                gid,
                GoalPriority::Normal,
                vec![EventSubscription::exact("brain.goal.completed")],
            )
            .await;
        let matched = scheduler.match_event("unrelated.event").await;
        assert!(matched.is_empty());
    }

    #[tokio::test]
    async fn due_goals_returns_expired_timers() {
        let scheduler = GoalScheduler::new();
        let gid = GoalId::new();
        scheduler.register(gid, GoalPriority::Normal, vec![]).await;
        // Newly registered goal should already be due (next_review = now)
        let due = scheduler.due_goals().await;
        assert_eq!(due, vec![gid]);
    }

    #[tokio::test]
    async fn reset_timer_postpones_review() {
        let scheduler = GoalScheduler::new();
        let gid = GoalId::new();
        scheduler.register(gid, GoalPriority::Normal, vec![]).await;
        scheduler.reset_timer(&gid).await;
        // After reset, next_review is in the future
        let due = scheduler.due_goals().await;
        assert!(due.is_empty());
    }

    #[tokio::test]
    async fn update_priority_changes_interval() {
        let scheduler = GoalScheduler::new();
        let gid = GoalId::new();
        scheduler.register(gid, GoalPriority::Low, vec![]).await;
        let interval_before = {
            let s = scheduler.schedules.read().await;
            s.get(&gid).unwrap().review_interval_ms
        };
        assert_eq!(interval_before, 60_000);
        scheduler
            .update_priority(&gid, GoalPriority::Critical)
            .await;
        let interval_after = {
            let s = scheduler.schedules.read().await;
            s.get(&gid).unwrap().review_interval_ms
        };
        assert_eq!(interval_after, 500);
    }

    #[tokio::test]
    async fn add_subscription_to_existing_goal() {
        let scheduler = GoalScheduler::new();
        let gid = GoalId::new();
        scheduler.register(gid, GoalPriority::Normal, vec![]).await;
        scheduler
            .add_subscription(&gid, EventSubscription::exact("brain.model.updated"))
            .await;
        let matched = scheduler.match_event("brain.model.updated").await;
        assert_eq!(matched, vec![gid]);
    }
}
