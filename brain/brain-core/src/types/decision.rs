use serde::{Deserialize, Serialize};

/// A decision produced by the cognitive loop's "decide" phase.
///
/// Captures the intent, target, reasoning, and confidence of a decision
/// made by the brain layer. This is a first-class value that bridges
/// the World Model (what is) to the Execution layer (what to do).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Decision {
    /// Continue observing without acting.
    Wait,
    /// Communicate a message (e.g., to the user or another agent).
    Communicate {
        recipient: String,
        message: String,
        reason: String,
    },
    /// Create or update an entity in the World Model.
    UpdateMemory {
        entity_name: String,
        entity_type: String,
        properties: Vec<(String, String)>,
        reason: String,
    },
    /// Execute a specific action via the execution platform.
    Execute {
        action: String,
        params: Vec<(String, String)>,
        reason: String,
    },
}
