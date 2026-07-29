#![forbid(unsafe_code)]

/// Continuous Cognitive Loop — observe, interpret, update, reflect, decide, learn.
pub mod cognitive_loop;

/// Companion Host — persistent personal AI with WM save/load and lifecycle management.
pub mod companion_host;

/// World Evolution Engine — evolves entity/relationship state over time.
pub mod evolution;

/// Cognitive Attention & Rhythm System — evaluates whether the AI should reflect.
pub mod attention;

pub mod agent_registry;
pub mod errors;
pub mod event_driver;
pub mod event_replay;
pub mod goal_scheduler;
pub mod notifications;
pub mod orchestrator;
pub mod recovery;
pub mod runtime;
pub mod strategy_engine;
pub mod types;
pub mod world_model;

pub use attention::{
    AttentionDecision, AttentionEvaluator, AttentionMemory, AttentionOutcome, Rhythm, UserResponse,
};
pub use cognitive_loop::CognitiveLoopService;
pub use companion_host::ui::{CompanionUi, UiConfig};
pub use companion_host::{
    CompanionHost, CompanionSettings, NotificationSettings, ObservationConfig,
    ObservationDebugInfo, ObservationEvent, ObservationLoop, ObservationSettings,
    PersistenceManager, SettingsManager,
};
pub use errors::{CoordinatorError, CoordinatorResult};
pub use event_driver::{BRAIN_EVENT_TYPES, EventDriver, RuntimeEvent};
pub use event_replay::{EventJournal, EventReplayManager, RecordedEvent};
pub use evolution::EvolutionEngine;
pub use goal_scheduler::{EventSubscription, GoalSchedule, GoalScheduler};
pub use notifications::{
    Notification, NotificationCategory, NotificationLevel, NotificationService,
};
pub use orchestrator::BrainOrchestrator;
pub use recovery::RecoveryManager;
pub use runtime::{AutonomousRuntime, BackgroundWorker, RuntimeStatus};
pub use types::{BrainSession, BrainState, CognitiveLoad, CoordinationReport, ProcessResult};

#[cfg(test)]
mod tests;
