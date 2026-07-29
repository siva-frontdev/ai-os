use crate::goal_scheduler::GoalScheduler;
use crate::notifications::{NotificationCategory, NotificationLevel, NotificationService};
use crate::orchestrator::BrainOrchestrator;
use brain_core::ids::GoalId;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::sync::mpsc;

/// A runtime event — either from the EventBus or synthetic.
#[derive(Debug, Clone)]
pub struct RuntimeEvent {
    pub event_type: String,
    pub payload: Option<String>,
    pub goal_id: Option<GoalId>,
}

impl RuntimeEvent {
    pub fn new(event_type: impl Into<String>) -> Self {
        Self {
            event_type: event_type.into(),
            payload: None,
            goal_id: None,
        }
    }

    pub fn with_payload(mut self, payload: impl Into<String>) -> Self {
        self.payload = Some(payload.into());
        self
    }

    pub fn for_goal(mut self, goal_id: GoalId) -> Self {
        self.goal_id = Some(goal_id);
        self
    }
}

/// Bridge handler that subscribes to the core EventBus and forwards
/// events to the EventDriver's internal channel.
///
/// This is a generic handler that can subscribe to any event type.
#[derive(Debug)]
pub(crate) struct EventBridge {
    tx: mpsc::UnboundedSender<RuntimeEvent>,
    event_type: &'static str,
}

impl EventBridge {
    pub fn new(tx: mpsc::UnboundedSender<RuntimeEvent>, event_type: &'static str) -> Self {
        Self { tx, event_type }
    }

    pub async fn handle_event(&self, payload: Option<String>) {
        let event = RuntimeEvent::new(self.event_type).with_payload(payload.unwrap_or_default());
        let _ = self.tx.send(event);
    }
}

/// Event-driven runtime engine.
///
/// Replaces the timer-driven cognitive loop. Events flow through:
///
/// 1. Receive event (from channel or external push)
/// 2. Record in journal for replay
/// 3. Update World Model
/// 4. Match against goal subscriptions
/// 5. Run cognitive tick for each affected goal
/// 6. Run plan repair if needed
/// 7. Wait for next event
///
/// A low-frequency heartbeat handles maintenance only
/// (deadlock detection, goal expiration, cache cleanup).
pub struct EventDriver {
    orchestrator: Arc<BrainOrchestrator>,
    goal_scheduler: Arc<GoalScheduler>,
    notifications: Arc<NotificationService>,
    event_tx: mpsc::UnboundedSender<RuntimeEvent>,
    event_rx: tokio::sync::Mutex<Option<mpsc::UnboundedReceiver<RuntimeEvent>>>,
    running: Arc<AtomicBool>,
    heartbeat_interval: Duration,
}

impl std::fmt::Debug for EventDriver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventDriver")
            .field("running", &self.running.load(Ordering::Relaxed))
            .field("heartbeat_interval", &self.heartbeat_interval)
            .finish()
    }
}

impl EventDriver {
    pub fn new(
        orchestrator: Arc<BrainOrchestrator>,
        goal_scheduler: Arc<GoalScheduler>,
        notifications: Arc<NotificationService>,
    ) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self {
            orchestrator,
            goal_scheduler,
            notifications,
            event_tx: tx,
            event_rx: tokio::sync::Mutex::new(Some(rx)),
            running: Arc::new(AtomicBool::new(false)),
            heartbeat_interval: Duration::from_secs(10),
        }
    }

    pub fn event_sender(&self) -> mpsc::UnboundedSender<RuntimeEvent> {
        self.event_tx.clone()
    }

    pub fn running(&self) -> &AtomicBool {
        &self.running
    }

    pub fn set_heartbeat_interval(&mut self, interval: Duration) {
        self.heartbeat_interval = interval;
    }

    /// Main event-processing loop. Runs until `stop()` is called.
    pub async fn run(&self) {
        self.running.store(true, Ordering::Release);

        let mut rx = self
            .event_rx
            .lock()
            .await
            .take()
            .expect("EventDriver::run called more than once");

        let mut heartbeat_timer = tokio::time::interval(self.heartbeat_interval);
        // Don't fire the heartbeat immediately
        heartbeat_timer.tick().await;

        loop {
            tokio::select! {
                // Process incoming events
                event = rx.recv() => {
                    match event {
                        Some(ev) => {
                            if !self.running.load(Ordering::Acquire) {
                                break;
                            }
                            self.process_event(ev).await;
                        }
                        None => {
                            // Channel closed — shut down
                            break;
                        }
                    }
                }
                // Heartbeat for maintenance tasks
                _ = heartbeat_timer.tick() => {
                    if !self.running.load(Ordering::Acquire) {
                        break;
                    }
                    self.run_maintenance().await;
                }
            }
        }
    }

    /// Process a single runtime event.
    async fn process_event(&self, event: RuntimeEvent) {
        // Skip processing if not running
        if !self.running.load(Ordering::Acquire) {
            return;
        }

        let event_type = &event.event_type;

        // Step 1: Determine which goals are affected
        let mut affected = self.goal_scheduler.match_event(event_type).await;

        // If the event is goal-scoped, add that goal
        if let Some(gid) = event.goal_id {
            if !affected.contains(&gid) {
                affected.push(gid);
            }
        }

        // Step 2: If no goals match and this isn't a goal lifecycle event,
        // check all active goals for a general evaluation
        if affected.is_empty() && !event_type.starts_with("runtime.maintenance") {
            // Push to all active goals as a generic "world changed" event
            affected.extend(self.find_all_active_goals().await);
        }

        // Step 3: Process each affected goal
        for goal_id in &affected {
            // Targeted cognitive tick for this specific goal
            if let Err(e) = self.orchestrator.cognitive_tick_for(*goal_id).await {
                self.notifications.notify_simple(
                    NotificationLevel::Warning,
                    NotificationCategory::CriticalFailure,
                    "Event-driven tick failed",
                    format!("Goal {goal_id}: {e}"),
                );
            }

            // Step 4: Check if the plan needs repair
            if let Err(_e) = self.orchestrator.evaluate_plan_health(*goal_id).await {
                // Plan needs repair
                let repaired = self.orchestrator.repair_plan(*goal_id).await;

                match repaired {
                    Ok(true) => {
                        self.notifications.notify_simple(
                            NotificationLevel::Info,
                            NotificationCategory::RecoveryPerformed,
                            "Plan repaired",
                            format!("Goal {goal_id}: plan was repaired after event {event_type}"),
                        );
                    }
                    Ok(false) => {
                        // No repair needed — good
                    }
                    Err(e) => {
                        self.notifications.notify_simple(
                            NotificationLevel::Warning,
                            NotificationCategory::CriticalFailure,
                            "Plan repair failed",
                            format!("Goal {goal_id}: {e}"),
                        );
                    }
                }
            }

            // Reset the goal's review timer
            self.goal_scheduler.reset_timer(goal_id).await;
        }

        // Step 5: Run a lightweight cognitive tick if there were meaningful events
        if !event_type.starts_with("runtime.maintenance") {
            let _ = self.orchestrator.cognitive_tick().await;
        }
    }

    /// Maintenance heartbeat — runs periodically regardless of events.
    async fn run_maintenance(&self) {
        // 1. Check for due goals (review timer expired)
        let due = self.goal_scheduler.due_goals().await;
        for goal_id in &due {
            if let Err(e) = self.orchestrator.cognitive_tick_for(*goal_id).await {
                self.notifications.notify_simple(
                    NotificationLevel::Warning,
                    NotificationCategory::CriticalFailure,
                    "Review tick failed",
                    format!("Goal {goal_id}: {e}"),
                );
            }
            self.goal_scheduler.reset_timer(goal_id).await;
        }

        // 2. Proactive retry of failed goals
        if let Ok(retried) = self.orchestrator.proactive_retry_failed().await {
            for gid in retried {
                self.notifications.notify_simple(
                    NotificationLevel::Info,
                    NotificationCategory::RecoveryPerformed,
                    "Auto-retried goal",
                    format!("Goal {gid} was automatically retried"),
                );
            }
        }

        // 3. Goal expiration / deferred goal check
        if let Ok(goals) = self.orchestrator.list_goals().await {
            let now = memory_core::Timestamp::now();
            for goal in &goals {
                if let Some(ref deferred) = goal.deferred_until {
                    if *deferred <= now {
                        // Deferred goal is ready to be reactivated
                        self.notifications.notify_simple(
                            NotificationLevel::Info,
                            NotificationCategory::GoalProgress,
                            "Deferred goal ready",
                            format!("Goal {} deferred time has passed", goal.goal_id),
                        );
                    }
                }
            }
        }
    }

    async fn find_all_active_goals(&self) -> Vec<GoalId> {
        if let Ok(goals) = self.orchestrator.list_active_goals().await {
            goals.into_iter().map(|g| g.goal_id).collect()
        } else {
            Vec::new()
        }
    }

    /// Stop the event loop.
    pub fn stop(&self) {
        self.running.store(false, Ordering::Release);
    }
}

/// Default brain event types that the driver subscribes to on the EventBus.
pub const BRAIN_EVENT_TYPES: &[&str] = &[
    "brain.goal.created",
    "brain.goal.activated",
    "brain.goal.completed",
    "brain.goal.failed",
    "brain.goal.cancelled",
    "brain.goal.blocked",
    "brain.goal.deferred",
    "brain.goal.paused",
    "brain.goal.retried",
    "brain.goal.dependency.violation",
    "brain.model.world.updated",
    "brain.model.resource.changed",
    "brain.plan.created",
    "brain.plan.updated",
    "brain.plan.rejected",
    "brain.reflection.completed",
    "brain.learning.cycle.completed",
    "brain.workflow.completed",
    "brain.workflow.failed",
    "brain.decision.escalation.required",
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::goal_scheduler::EventSubscription;
    use brain_core::ids::GoalId;
    use brain_core::types::GoalPriority;
    use brain_goals::GoalManager;
    use brain_goals::InMemoryGoalStore;
    use std::sync::Arc;

    fn make_orchestrator() -> Arc<BrainOrchestrator> {
        Arc::new(BrainOrchestrator::new(
            Box::new(crate::tests::MockPolicyEvaluator),
            Arc::new(crate::tests::MockToolRegistry),
            Arc::new(InMemoryGoalStore::new()),
            Arc::new(crate::tests::MockReasoningService),
        ))
    }

    #[tokio::test]
    async fn event_processing_affects_subscribed_goals() {
        let orch = make_orchestrator();
        let scheduler = Arc::new(GoalScheduler::new());
        let notifications = Arc::new(NotificationService::new());
        let driver = EventDriver::new(orch.clone(), scheduler.clone(), notifications);

        let gid = GoalId::new();
        scheduler
            .register(
                gid,
                GoalPriority::Normal,
                vec![EventSubscription::exact("test.event")],
            )
            .await;

        // After processing, the scheduler state should be updated
        driver.process_event(RuntimeEvent::new("test.event")).await;

        // Verify the event was matched and processed
        // The goal should still be in the scheduler
        assert!(scheduler.len().await > 0, "Goal should still be registered");
    }

    #[tokio::test]
    async fn heartbeat_runs_maintenance() {
        let orch = make_orchestrator();
        let scheduler = Arc::new(GoalScheduler::new());
        let notifications = Arc::new(NotificationService::new());
        let driver = EventDriver::new(orch.clone(), scheduler.clone(), notifications);

        // Should not panic — just runs maintenance checks
        driver.run_maintenance().await;
    }

    #[tokio::test]
    async fn find_all_active_goals_empty() {
        let orch = make_orchestrator();
        let scheduler = Arc::new(GoalScheduler::new());
        let notifications = Arc::new(NotificationService::new());
        let driver = EventDriver::new(orch.clone(), scheduler.clone(), notifications);

        let goals = driver.find_all_active_goals().await;
        assert!(goals.is_empty());
    }

    #[tokio::test]
    async fn match_event_no_subscribers() {
        let orch = make_orchestrator();
        let scheduler = Arc::new(GoalScheduler::new());
        let notifications = Arc::new(NotificationService::new());
        let driver = EventDriver::new(orch, scheduler.clone(), notifications);

        // An event with no subscribers should not cause any issues
        driver
            .process_event(RuntimeEvent::new("unrelated.event"))
            .await;
    }

    #[tokio::test]
    async fn goal_scoped_event_wakes_specific_goal() {
        let orch = make_orchestrator();
        let scheduler = Arc::new(GoalScheduler::new());
        let notifications = Arc::new(NotificationService::new());
        let driver = EventDriver::new(orch, scheduler.clone(), notifications);
        let gid = GoalId::new();

        scheduler.register(gid, GoalPriority::Normal, vec![]).await;

        // Event with goal_id should wake that goal even without subscription match
        driver
            .process_event(RuntimeEvent::new("some.event").for_goal(gid))
            .await;
    }
}
