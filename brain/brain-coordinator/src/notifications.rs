use brain_core::ids::GoalId;
use memory_core::Timestamp;
use serde::{Deserialize, Serialize};
use std::sync::RwLock;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NotificationLevel {
    Info,
    Success,
    Warning,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NotificationCategory {
    GoalCompleted,
    ApprovalRequired,
    CriticalFailure,
    StrategyUpdated,
    ResourceExhaustion,
    GoalProgress,
    GoalBlocked,
    GoalDeferred,
    GoalCreated,
    ExternalEvent,
    RecoveryPerformed,
    LearningInsight,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub id: Uuid,
    pub level: NotificationLevel,
    pub category: NotificationCategory,
    pub title: String,
    pub message: String,
    pub goal_id: Option<GoalId>,
    pub timestamp: Timestamp,
    pub acknowledged: bool,
    pub expires_at: Option<Timestamp>,
}

impl Notification {
    pub fn new(
        level: NotificationLevel,
        category: NotificationCategory,
        title: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            level,
            category,
            title: title.into(),
            message: message.into(),
            goal_id: None,
            timestamp: Timestamp::now(),
            acknowledged: false,
            expires_at: None,
        }
    }

    pub fn for_goal(mut self, goal_id: GoalId) -> Self {
        self.goal_id = Some(goal_id);
        self
    }

    pub fn expires(mut self, at: Timestamp) -> Self {
        self.expires_at = Some(at);
        self
    }
}

#[derive(Debug)]
pub struct NotificationService {
    notifications: RwLock<Vec<Notification>>,
    max_pending: usize,
}

impl NotificationService {
    pub fn new() -> Self {
        Self {
            notifications: RwLock::new(Vec::new()),
            max_pending: 100,
        }
    }

    pub fn with_max_pending(mut self, max: usize) -> Self {
        self.max_pending = max;
        self
    }

    pub fn notify(&self, notification: Notification) {
        let mut guard = self.notifications.write().expect("notifications lock");
        guard.push(notification);
        if guard.len() > self.max_pending {
            guard.remove(0);
        }
    }

    pub fn notify_simple(
        &self,
        level: NotificationLevel,
        category: NotificationCategory,
        title: impl Into<String>,
        message: impl Into<String>,
    ) {
        self.notify(Notification::new(level, category, title, message));
    }

    pub fn acknowledge(&self, id: Uuid) -> bool {
        let mut guard = self.notifications.write().expect("notifications lock");
        if let Some(n) = guard.iter_mut().find(|n| n.id == id) {
            n.acknowledged = true;
            true
        } else {
            false
        }
    }

    pub fn pending(&self) -> Vec<Notification> {
        let guard = self.notifications.read().expect("notifications lock");
        let now = Timestamp::now();
        guard
            .iter()
            .filter(|n| !n.acknowledged && n.expires_at.is_none_or(|exp| exp > now))
            .cloned()
            .collect()
    }

    pub fn all(&self) -> Vec<Notification> {
        let guard = self.notifications.read().expect("notifications lock");
        guard.clone()
    }

    pub fn for_goal(&self, goal_id: &GoalId) -> Vec<Notification> {
        let guard = self.notifications.read().expect("notifications lock");
        guard
            .iter()
            .filter(|n| n.goal_id == Some(*goal_id))
            .cloned()
            .collect()
    }

    pub fn unacknowledged_count(&self) -> usize {
        let guard = self.notifications.read().expect("notifications lock");
        let now = Timestamp::now();
        guard
            .iter()
            .filter(|n| !n.acknowledged && n.expires_at.is_none_or(|exp| exp > now))
            .count()
    }
}

impl Default for NotificationService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notify_and_pending() {
        let svc = NotificationService::new();
        assert_eq!(svc.pending().len(), 0);

        svc.notify_simple(
            NotificationLevel::Info,
            NotificationCategory::GoalCompleted,
            "done",
            "goal finished",
        );
        assert_eq!(svc.pending().len(), 1);

        let n = svc.pending().remove(0);
        assert_eq!(n.title, "done");
        assert!(!n.acknowledged);

        assert!(svc.acknowledge(n.id));
        assert_eq!(svc.pending().len(), 0);
    }

    #[test]
    fn test_notification_expiry() {
        let svc = NotificationService::new();
        let past_nanos = Timestamp::now().as_nanos().saturating_sub(10_000_000_000);
        let past = Timestamp::from_nanos(past_nanos);
        svc.notify(
            Notification::new(
                NotificationLevel::Info,
                NotificationCategory::ExternalEvent,
                "expired",
                "already gone",
            )
            .expires(past),
        );
        assert_eq!(svc.pending().len(), 0);
    }

    #[test]
    fn test_goal_scoped_notifications() {
        let svc = NotificationService::new();
        let gid = GoalId::new();
        svc.notify(
            Notification::new(
                NotificationLevel::Success,
                NotificationCategory::GoalCompleted,
                "g1",
                "goal one",
            )
            .for_goal(gid),
        );
        svc.notify_simple(
            NotificationLevel::Info,
            NotificationCategory::GoalCreated,
            "nog",
            "no goal",
        );
        assert_eq!(svc.for_goal(&gid).len(), 1);
    }

    #[test]
    fn test_max_pending_eviction() {
        let svc = NotificationService::new().with_max_pending(3);
        for i in 0..5 {
            svc.notify_simple(
                NotificationLevel::Info,
                NotificationCategory::ExternalEvent,
                format!("n{}", i),
                format!("msg {}", i),
            );
        }
        assert_eq!(svc.all().len(), 3);
        assert_eq!(svc.all().first().unwrap().title, "n2");
    }

    #[test]
    fn test_unacknowledged_count() {
        let svc = NotificationService::new();
        assert_eq!(svc.unacknowledged_count(), 0);
        svc.notify_simple(
            NotificationLevel::Warning,
            NotificationCategory::ResourceExhaustion,
            "warn",
            "low resources",
        );
        assert_eq!(svc.unacknowledged_count(), 1);
    }
}
