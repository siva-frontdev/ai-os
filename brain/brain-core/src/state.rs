//! `BrainStateMachine` — validates platform-level FSM transitions.
use crate::types::BrainState;

/// Validates transitions between `BrainState` values.
///
/// The state machine lives in `brain-core` as a pure function. `Coordinator`
/// calls `validate_transition` before acquiring the write lock on its state.
#[derive(Debug, Clone, Copy, Default)]
pub struct BrainStateMachine;

impl BrainStateMachine {
    pub fn new() -> Self { Self }

    /// Validate a planned state transition.
    ///
    /// Returns `Ok(())` if the transition is allowed, or an `Err(String)`
    /// describing the invalid transition.
    pub fn validate_transition(&self, from: BrainState, to: BrainState) -> Result<(), String> {
        if from == to { return Ok(()); }
        match (from, to) {
            (BrainState::Sleeping, BrainState::Idle) => Ok(()),
            (BrainState::Idle, BrainState::Planning) => Ok(()),
            (BrainState::Planning, BrainState::Reasoning) => Ok(()),
            (BrainState::Reasoning, BrainState::WaitingModel) => Ok(()),
            (BrainState::WaitingModel, BrainState::Planning) => Ok(()),
            (BrainState::WaitingModel, BrainState::WaitingExecution) => Ok(()),
            (BrainState::WaitingExecution, BrainState::Reflecting) => Ok(()),
            (BrainState::Reflecting, BrainState::Learning) => Ok(()),
            (BrainState::Learning, BrainState::Idle) => Ok(()),
            (_, BrainState::Recovering) => Ok(()),
            (BrainState::Recovering, BrainState::Idle) => Ok(()),
            (_, BrainState::Paused) => Ok(()),
            (BrainState::Paused, BrainState::Idle) => Ok(()),
            (_, BrainState::Stopping) => Ok(()),
            (BrainState::Stopping, BrainState::Stopped) => Ok(()),
            _ => Err(format!("invalid brain state transition: {:?} -> {:?}", from, to)),
        }
    }
}
