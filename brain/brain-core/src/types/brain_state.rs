//! `BrainState` — platform-level finite state machine (unit variants, all public).
use serde::{Deserialize, Serialize};

/// States of the Brain Platform FSM owned by `Coordinator`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BrainState {
    Sleeping,
    Idle,
    Planning,
    Reasoning,
    WaitingModel,
    WaitingExecution,
    Reflecting,
    Learning,
    Recovering,
    Paused,
    Stopping,
    Stopped,
}

impl BrainState {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Sleeping => "Sleeping",
            Self::Idle => "Idle",
            Self::Planning => "Planning",
            Self::Reasoning => "Reasoning",
            Self::WaitingModel => "WaitingModel",
            Self::WaitingExecution => "WaitingExecution",
            Self::Reflecting => "Reflecting",
            Self::Learning => "Learning",
            Self::Recovering => "Recovering",
            Self::Paused => "Paused",
            Self::Stopping => "Stopping",
            Self::Stopped => "Stopped",
        }
    }
}

impl std::fmt::Display for BrainState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
