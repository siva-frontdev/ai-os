//! Coordinator / lifecycle events.
use crate::types::BrainState;
use memory_core::Timestamp;
use ai_os_core::events::Event;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BrainStarted {
    pub version: String,
    pub subsystems_loaded: Vec<String>,
    pub active_sessions: u32,
    pub started_at: Timestamp,
}

impl Event for BrainStarted {
    fn event_type(&self) -> &'static str { "brain.started" }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BrainPaused {
    pub reason: Option<String>,
    pub active_goals_preserved: u32,
    pub paused_at: Timestamp,
}

impl Event for BrainPaused {
    fn event_type(&self) -> &'static str { "brain.paused" }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BrainResumed {
    pub previous_state: BrainState,
    pub resumed_at: Timestamp,
}

impl Event for BrainResumed {
    fn event_type(&self) -> &'static str { "brain.resumed" }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BrainStopping {
    pub reason: String, // "normal_shutdown", "error", "operator"
    pub stopping_at: Timestamp,
}

impl Event for BrainStopping {
    fn event_type(&self) -> &'static str { "brain.stopping" }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BrainStopped {
    pub uptime_ms: u64,
    pub goals_completed: u32,
    pub goals_failed: u32,
    pub stopped_at: Timestamp,
}

impl Event for BrainStopped {
    fn event_type(&self) -> &'static str { "brain.stopped" }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BrainFailed {
    pub error: String,
    pub recovery_attempts: u32,
    pub state_at_failure: BrainState,
    pub failed_at: Timestamp,
}

impl Event for BrainFailed {
    fn event_type(&self) -> &'static str { "brain.failed" }
}
