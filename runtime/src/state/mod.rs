//! # Runtime State
//!
//! Finite state machine for the runtime lifecycle.
//!
//! ## State diagram
//!
//! ```text
//!        ┌──────────┐
//!        │ Created  │
//!        └────┬─────┘
//!             │ init
//!        ┌────▼──────┐
//!        │Initializing│
//!        └────┬───────┘
//!             │ start
//!        ┌────▼────┐
//!        │ Running │◄────────────┐
//!        └────┬────┘             │
//!             │ drain            │ resume
//!        ┌────▼──────┐           │
//!        │ Draining  │───────────┘
//!        └────┬──────┘  (auto-restart)
//!             │ stop
//!        ┌────▼────┐
//!        │ Stopped │
//!        └─────────┘
//! ```
//!
//! Invalid transitions return [`RuntimeError::State`].
use std::fmt::Debug;
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};

use crate::error::RuntimeError;

// ── Runtime phase ────────────────────────────────────────

/// Runtime lifecycle phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuntimePhase {
    Created,
    Initializing,
    Running,
    Draining,
    Stopped,
    Failed,
}

impl RuntimePhase {
    pub fn is_terminal(&self) -> bool {
        matches!(self, RuntimePhase::Stopped | RuntimePhase::Failed)
    }

    pub fn is_active(&self) -> bool {
        matches!(self, RuntimePhase::Running)
    }
}

// ── Valid transitions ────────────────────────────────────

static TRANSITIONS: &[(RuntimePhase, RuntimePhase)] = &[
    (RuntimePhase::Created, RuntimePhase::Initializing),
    (RuntimePhase::Initializing, RuntimePhase::Running),
    (RuntimePhase::Initializing, RuntimePhase::Failed),
    (RuntimePhase::Running, RuntimePhase::Draining),
    (RuntimePhase::Running, RuntimePhase::Failed),
    (RuntimePhase::Draining, RuntimePhase::Running), // resume
    (RuntimePhase::Draining, RuntimePhase::Stopped),
    (RuntimePhase::Draining, RuntimePhase::Failed),
];

fn valid_transition(from: RuntimePhase, to: RuntimePhase) -> bool {
    if from == to {
        return true;
    }
    TRANSITIONS.contains(&(from, to))
}

// ── Runtime state machine ────────────────────────────────

/// Thread-safe runtime state machine.
pub trait RuntimeStateMachine: Debug + Send + Sync {
    fn phase(&self) -> RuntimePhase;
    fn transition(&self, to: RuntimePhase) -> Result<RuntimePhase, RuntimeError>;
    fn can_transition_to(&self, to: RuntimePhase) -> bool;
    fn is_running(&self) -> bool;
}

// ── Implementation ───────────────────────────────────────

#[derive(Debug)]
pub struct DefaultRuntimeState {
    phase: RwLock<RuntimePhase>,
}

impl DefaultRuntimeState {
    pub fn new() -> Self {
        Self {
            phase: RwLock::new(RuntimePhase::Created),
        }
    }
}

impl Default for DefaultRuntimeState {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeStateMachine for DefaultRuntimeState {
    fn phase(&self) -> RuntimePhase {
        match self.phase.read() {
            Ok(guard) => *guard,
            Err(_) => RuntimePhase::Failed,
        }
    }

    fn transition(&self, to: RuntimePhase) -> Result<RuntimePhase, RuntimeError> {
        let mut guard = self
            .phase
            .write()
            .map_err(|_| RuntimeError::State("lock poisoned".into()))?;
        let from = *guard;
        if !valid_transition(from, to) {
            return Err(RuntimeError::State(format!(
                "invalid transition: {:?} → {:?}",
                from, to
            )));
        }
        *guard = to;
        Ok(from)
    }

    fn can_transition_to(&self, to: RuntimePhase) -> bool {
        let from = self.phase();
        valid_transition(from, to)
    }

    fn is_running(&self) -> bool {
        self.phase() == RuntimePhase::Running
    }
}

// ── Event ─────────────────────────────────────────────────

/// Fired when the runtime phase changes.
#[derive(Debug, Clone)]
pub struct RuntimePhaseChanged {
    pub from: RuntimePhase,
    pub to: RuntimePhase,
}

impl ai_os_core::events::Event for RuntimePhaseChanged {
    fn event_type(&self) -> &'static str {
        "runtime.phase_changed"
    }
}

// ── Tests ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_phase_is_created() {
        let s = DefaultRuntimeState::new();
        assert_eq!(s.phase(), RuntimePhase::Created);
    }

    #[test]
    fn valid_created_to_initializing() {
        let s = DefaultRuntimeState::new();
        let from = s.transition(RuntimePhase::Initializing).unwrap();
        assert_eq!(from, RuntimePhase::Created);
        assert_eq!(s.phase(), RuntimePhase::Initializing);
    }

    #[test]
    fn invalid_created_to_running_fails() {
        let s = DefaultRuntimeState::new();
        let result = s.transition(RuntimePhase::Running);
        assert!(result.is_err());
    }

    #[test]
    fn full_lifecycle() {
        let s = DefaultRuntimeState::new();
        s.transition(RuntimePhase::Initializing).unwrap();
        s.transition(RuntimePhase::Running).unwrap();
        s.transition(RuntimePhase::Draining).unwrap();
        s.transition(RuntimePhase::Stopped).unwrap();
        assert_eq!(s.phase(), RuntimePhase::Stopped);
        assert!(s.phase().is_terminal());
    }

    #[test]
    fn drain_to_running_resume() {
        let s = DefaultRuntimeState::new();
        s.transition(RuntimePhase::Initializing).unwrap();
        s.transition(RuntimePhase::Running).unwrap();
        s.transition(RuntimePhase::Draining).unwrap();
        assert!(s.can_transition_to(RuntimePhase::Running));
        s.transition(RuntimePhase::Running).unwrap();
        assert!(s.is_running());
    }

    #[test]
    fn can_transition_to_checks() {
        let s = DefaultRuntimeState::new();
        assert!(s.can_transition_to(RuntimePhase::Initializing));
        assert!(!s.can_transition_to(RuntimePhase::Running));
        assert!(!s.can_transition_to(RuntimePhase::Stopped));
    }
}
