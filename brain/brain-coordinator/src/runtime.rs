use crate::errors::{CoordinatorError, CoordinatorResult};
use crate::event_driver::EventDriver;
use crate::event_replay::EventJournal;
use crate::goal_scheduler::{EventSubscription, GoalScheduler};
use crate::notifications::{
    Notification, NotificationCategory, NotificationLevel, NotificationService,
};
use crate::orchestrator::BrainOrchestrator;
use crate::recovery::RecoveryManager;

use brain_core::types::GoalPriority;
use brain_goals::GoalManager;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::sync::RwLock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeStatus {
    Stopped,
    Running,
    Paused,
    Failed(String),
}

struct RuntimeState {
    running: AtomicBool,
    paused: AtomicBool,
}

/// Background worker that runs on a fixed-interval schedule.
#[derive(Clone)]
pub struct BackgroundWorker {
    pub name: &'static str,
    pub interval: Duration,
    pub enabled: bool,
}

impl BackgroundWorker {
    pub const fn new(name: &'static str, interval: Duration) -> Self {
        Self {
            name,
            interval,
            enabled: true,
        }
    }

    pub const fn disabled(name: &'static str, interval: Duration) -> Self {
        Self {
            name,
            interval,
            enabled: false,
        }
    }
}

/// Event-Driven Autonomous Runtime.
///
/// Replaces the timer-driven cognitive loop with an event-driven
/// architecture. The runtime reacts to events from the orchestrator
/// and external sources rather than polling every N seconds.
///
/// # Lifecycle
///
/// 1. **`start`** — recovers interrupted goals, wires the event
///    channel, spawns the event driver, heartbeat, and workers.
/// 2. **`stop`** — gracefully shuts down all loops.
///
/// # Architecture
///
/// - **EventDriver** — processes incoming events, matches them
///   against goal subscriptions, triggers targeted cognitive ticks.
/// - **GoalScheduler** — maintains per-goal event subscriptions and
///   adaptive review timers.
/// - **Heartbeat** — low-frequency (30 s) maintenance only:
///   deadlock detection, goal expiration, proactive retry.
/// - **Background workers** — still timer-driven but with longer
///   intervals and reduced scope.
pub struct AutonomousRuntime {
    orchestrator: Arc<BrainOrchestrator>,
    goal_manager: Arc<GoalManager>,
    notifications: Arc<NotificationService>,
    workers: Vec<BackgroundWorker>,
    state: Arc<RuntimeState>,
    status: RwLock<RuntimeStatus>,
    goal_scheduler: Arc<GoalScheduler>,
    event_journal: Arc<EventJournal>,
    heartbeat_interval: Duration,
}

impl std::fmt::Debug for AutonomousRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AutonomousRuntime")
            .field(
                "workers",
                &self.workers.iter().map(|w| w.name).collect::<Vec<_>>(),
            )
            .field("running", &self.state.running.load(Ordering::Relaxed))
            .finish()
    }
}

impl AutonomousRuntime {
    /// Create a new autonomous runtime.
    pub fn new(orchestrator: Arc<BrainOrchestrator>, goal_manager: Arc<GoalManager>) -> Self {
        Self {
            notifications: Arc::new(NotificationService::new()),
            orchestrator,
            goal_manager,
            workers: Self::default_workers(),
            state: Arc::new(RuntimeState {
                running: AtomicBool::new(false),
                paused: AtomicBool::new(false),
            }),
            status: RwLock::new(RuntimeStatus::Stopped),
            goal_scheduler: Arc::new(GoalScheduler::new()),
            event_journal: Arc::new(EventJournal::new()),
            heartbeat_interval: Duration::from_secs(30),
        }
    }

    fn default_workers() -> Vec<BackgroundWorker> {
        vec![
            BackgroundWorker::new("goal_monitor", Duration::from_secs(30)),
            BackgroundWorker::new("resource_monitor", Duration::from_secs(60)),
            BackgroundWorker::new("reflection_scheduler", Duration::from_secs(120)),
            BackgroundWorker::new("learning_scheduler", Duration::from_secs(300)),
            BackgroundWorker::new("persistence", Duration::from_secs(600)),
        ]
    }

    /// Override the default set of background workers.
    pub fn with_workers(mut self, workers: Vec<BackgroundWorker>) -> Self {
        self.workers = workers;
        self
    }

    /// Customise the heartbeat interval.
    pub fn with_heartbeat_interval(mut self, d: Duration) -> Self {
        self.heartbeat_interval = d;
        self
    }

    /// Access the notification service.
    pub fn notifications(&self) -> &NotificationService {
        &self.notifications
    }

    /// Access the goal scheduler.
    pub fn goal_scheduler(&self) -> &GoalScheduler {
        &self.goal_scheduler
    }

    /// Access the event journal.
    pub fn event_journal(&self) -> &EventJournal {
        &self.event_journal
    }

    /// Current runtime status.
    pub async fn status(&self) -> RuntimeStatus {
        self.status.read().await.clone()
    }

    /// Start the autonomous runtime.
    ///
    /// Phase 1: Recover interrupted goals.
    /// Phase 2: Register active goals with the scheduler.
    /// Phase 3: Wire event channel and spawn EventDriver.
    /// Phase 4: Spawn heartbeat.
    /// Phase 5: Spawn background workers.
    pub async fn start(&self) -> CoordinatorResult<()> {
        if self.state.running.swap(true, Ordering::AcqRel) {
            return Err(CoordinatorError::Internal("runtime already running".into()));
        }

        *self.status.write().await = RuntimeStatus::Running;

        // Phase 1: Recover interrupted goals
        let recovered = RecoveryManager::resume_all(&self.goal_manager).await?;
        if !recovered.is_empty() {
            self.notifications.notify(Notification::new(
                NotificationLevel::Info,
                NotificationCategory::RecoveryPerformed,
                format!("Recovered {} goals after restart", recovered.len()),
                format!(
                    "The following goals were automatically resumed: {:?}",
                    recovered
                ),
            ));
        }

        // Phase 2: Register active goals with the scheduler
        if let Ok(goals) = self.orchestrator.list_active_goals().await {
            for goal in &goals {
                let subs = Self::default_subscriptions(&goal.goal_type);
                self.goal_scheduler
                    .register(goal.goal_id, goal.priority, subs)
                    .await;
            }
        }

        // Phase 3: Wire event channel and spawn EventDriver
        let event_driver = Arc::new(EventDriver::new(
            self.orchestrator.clone(),
            self.goal_scheduler.clone(),
            self.notifications.clone(),
        ));

        // Connect the orchestrator's event channel to the driver
        let _event_tx = event_driver.event_sender();
        // We need a way to inject the event_tx into the orchestrator.
        // Since the orchestrator is behind Arc, we use a channel
        // approach: the orchestrator stores an optional sender.
        // But we can't modify it after construction via Arc.
        // Instead, the DesktopAgent or caller should use
        // BrainOrchestrator::with_event_channel() during construction.
        // For goals registered here, we push synthetic events.
        // For now, the event_tx is available for external bridge code.

        let driver = event_driver.clone();
        let _driver_handle = tokio::spawn(async move {
            driver.run().await;
        });

        // Phase 4: Spawn heartbeat (maintenance only)
        let hb_orch = self.orchestrator.clone();
        let hb_sched = self.goal_scheduler.clone();
        let hb_nsvc = self.notifications.clone();
        let hb_state = self.state.clone();
        let hb_interval = self.heartbeat_interval;
        tokio::spawn(async move {
            let mut timer = tokio::time::interval(hb_interval);
            timer.tick().await;
            loop {
                timer.tick().await;
                if !hb_state.running.load(Ordering::Acquire) {
                    break;
                }
                if hb_state.paused.load(Ordering::Acquire) {
                    continue;
                }

                // Process due goals (review timer expired)
                let due = hb_sched.due_goals().await;
                for goal_id in &due {
                    if let Err(e) = hb_orch.cognitive_tick_for(*goal_id).await {
                        hb_nsvc.notify_simple(
                            NotificationLevel::Warning,
                            NotificationCategory::CriticalFailure,
                            "Heartbeat review failed",
                            format!("Goal {goal_id}: {e}"),
                        );
                    }
                    hb_sched.reset_timer(goal_id).await;
                }

                // Proactive retry of failed goals
                if let Ok(retried) = hb_orch.proactive_retry_failed().await {
                    for gid in retried {
                        hb_sched
                            .register(gid, GoalPriority::Normal, Self::default_subscriptions(""))
                            .await;
                        hb_nsvc.notify_simple(
                            NotificationLevel::Info,
                            NotificationCategory::RecoveryPerformed,
                            "Auto-retried goal",
                            format!("Goal {gid} was automatically retried"),
                        );
                    }
                }
            }
        });

        // Phase 5: Spawn background workers
        for worker in &self.workers {
            if !worker.enabled {
                continue;
            }
            let interval = worker.interval;
            let name = worker.name;
            let orch = self.orchestrator.clone();
            let nsvc = self.notifications.clone();
            let state = self.state.clone();

            tokio::spawn(async move {
                let mut timer = tokio::time::interval(interval);
                timer.tick().await;
                loop {
                    timer.tick().await;
                    if !state.running.load(Ordering::Acquire) {
                        break;
                    }
                    if state.paused.load(Ordering::Acquire) {
                        continue;
                    }
                    match name {
                        "goal_monitor" => Self::run_goal_monitor(&orch, &nsvc).await,
                        "resource_monitor" => Self::run_resource_monitor(&orch, &nsvc).await,
                        "reflection_scheduler" => Self::run_reflection_scheduler(&orch).await,
                        "learning_scheduler" => Self::run_learning_scheduler(&orch, &nsvc).await,
                        "persistence" => Self::run_persistence(&orch).await,
                        _ => {}
                    }
                }
            });
        }

        Ok(())
    }

    /// Default event subscriptions for a goal based on its type.
    fn default_subscriptions(goal_type: &str) -> Vec<EventSubscription> {
        let mut subs = Vec::new();

        // All goals care about their own lifecycle
        subs.push(EventSubscription::prefix("brain.goal."));
        subs.push(EventSubscription::exact("brain.model.world.updated"));

        // Goals with specific types get targeted subscriptions
        match goal_type {
            t if t.contains("deploy") || t.contains("Deploy") => {
                subs.push(EventSubscription::exact("runtime.task.completed"));
                subs.push(EventSubscription::exact("runtime.task.failed"));
            }
            t if t.contains("code") || t.contains("Code") || t.contains("develop") => {
                subs.push(EventSubscription::exact("brain.plan.updated"));
                subs.push(EventSubscription::exact("brain.reflection.completed"));
            }
            t if t.contains("monitor") || t.contains("Monitor") || t.contains("watch") => {
                subs.push(EventSubscription::prefix("brain.model."));
            }
            _ => {
                // Generic goals subscribe to workflow and task events
                subs.push(EventSubscription::prefix("brain.workflow."));
            }
        }

        subs
    }

    /// Gracefully stop the runtime.
    pub async fn stop(&self) -> CoordinatorResult<()> {
        self.state.running.store(false, Ordering::Release);
        *self.status.write().await = RuntimeStatus::Stopped;
        Ok(())
    }

    /// Pause the runtime (workers and loops skip ticks).
    pub async fn pause(&self) {
        self.state.paused.store(true, Ordering::Release);
        *self.status.write().await = RuntimeStatus::Paused;
    }

    /// Resume a paused runtime.
    pub async fn resume(&self) {
        self.state.paused.store(false, Ordering::Release);
        *self.status.write().await = RuntimeStatus::Running;
    }

    // ── Background worker implementations ─────────────────────

    async fn run_goal_monitor(orch: &BrainOrchestrator, nsvc: &NotificationService) {
        let goals = match orch.list_active_goals().await {
            Ok(g) => g,
            Err(_) => return,
        };
        for goal in &goals {
            if let Some(ref reason) = goal.blocked_reason {
                nsvc.notify(
                    Notification::new(
                        NotificationLevel::Warning,
                        NotificationCategory::GoalBlocked,
                        format!("Goal blocked: {}", goal.description),
                        reason.clone(),
                    )
                    .for_goal(goal.goal_id),
                );
            }
        }
    }

    async fn run_resource_monitor(orch: &BrainOrchestrator, nsvc: &NotificationService) {
        let state = orch.world_model().resource_state();
        match state {
            brain_core::model::ResourceState::Exhausted => {
                nsvc.notify_simple(
                    NotificationLevel::Critical,
                    NotificationCategory::ResourceExhaustion,
                    "Resources exhausted",
                    "All system resources are exhausted. Only human-assisted tasks can proceed.",
                );
            }
            brain_core::model::ResourceState::Critical => {
                nsvc.notify_simple(
                    NotificationLevel::Warning,
                    NotificationCategory::ResourceExhaustion,
                    "Resources critical",
                    "System resources are critically low. Parallel tasks will be degraded.",
                );
            }
            brain_core::model::ResourceState::Degraded
            | brain_core::model::ResourceState::Healthy => {}
        }
    }

    async fn run_reflection_scheduler(orch: &BrainOrchestrator) {
        let _ = orch;
    }

    async fn run_learning_scheduler(orch: &BrainOrchestrator, nsvc: &NotificationService) {
        if let Ok(feedback) = orch.get_learning_feedback() {
            if feedback.pattern_count > 0 {
                let applied = feedback.strategy_adjustments.len();
                let hints = feedback.planner_hints.len();
                if applied > 0 || hints > 0 {
                    nsvc.notify_simple(
                        NotificationLevel::Info,
                        NotificationCategory::LearningInsight,
                        "Learning cycle completed",
                        format!(
                            "Applied {} strategy adjustments, {} planner hints. Cognitive health: {:.2}",
                            applied, hints, feedback.cognitive_health
                        ),
                    );
                }
            }
        }
    }

    async fn run_persistence(orch: &BrainOrchestrator) {
        if let Ok(goals) = orch.list_goals().await {
            if goals.first().is_some() {
                let _ = orch.goal_manager().snapshot("autosave").await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_orchestrator() -> Arc<BrainOrchestrator> {
        Arc::new(BrainOrchestrator::new(
            Box::new(crate::tests::MockPolicyEvaluator),
            Arc::new(crate::tests::MockToolRegistry),
            Arc::new(brain_goals::memory_store::InMemoryGoalStore::new()),
            Arc::new(crate::tests::MockReasoningService),
        ))
    }

    fn make_goal_manager() -> Arc<GoalManager> {
        Arc::new(GoalManager::new(Arc::new(
            brain_goals::memory_store::InMemoryGoalStore::new(),
        )))
    }

    #[test]
    fn test_default_workers_configured() {
        let workers = AutonomousRuntime::default_workers();
        assert_eq!(workers.len(), 5);
        assert_eq!(workers[0].name, "goal_monitor");
        assert_eq!(workers[3].name, "learning_scheduler");
        assert_eq!(workers[4].name, "persistence");
    }

    #[tokio::test]
    async fn test_start_stop_cycle() {
        let rt = AutonomousRuntime::new(make_orchestrator(), make_goal_manager());
        assert_eq!(rt.status().await, RuntimeStatus::Stopped);
        rt.start().await.unwrap();
        assert_eq!(rt.status().await, RuntimeStatus::Running);
        rt.stop().await.unwrap();
        assert_eq!(rt.status().await, RuntimeStatus::Stopped);
    }

    #[tokio::test]
    async fn test_pause_resume() {
        let rt = AutonomousRuntime::new(make_orchestrator(), make_goal_manager());
        rt.start().await.unwrap();
        rt.pause().await;
        assert_eq!(rt.status().await, RuntimeStatus::Paused);
        rt.resume().await;
        assert_eq!(rt.status().await, RuntimeStatus::Running);
        rt.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_double_start_rejected() {
        let rt = AutonomousRuntime::new(make_orchestrator(), make_goal_manager());
        rt.start().await.unwrap();
        let result = rt.start().await;
        assert!(result.is_err());
        rt.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_default_subscriptions_for_deploy() {
        let subs = AutonomousRuntime::default_subscriptions("deploy website");
        let has_task_completed = subs.iter().any(|s| s.matches("runtime.task.completed"));
        assert!(has_task_completed);
    }

    #[tokio::test]
    async fn test_default_subscriptions_for_code() {
        let subs = AutonomousRuntime::default_subscriptions("code review");
        let has_plan_updated = subs.iter().any(|s| s.matches("brain.plan.updated"));
        assert!(has_plan_updated);
    }
}
