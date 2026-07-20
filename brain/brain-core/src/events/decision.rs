//! Decision events.
use crate::ids::{DecisionId, PlanId};
use crate::types::Confidence;
use ai_os_core::events::Event;
use memory_core::Timestamp;
use serde::{Deserialize, Serialize};

/// Published when a decision is made.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DecisionMade {
    pub decision_id: DecisionId,
    pub plan_id: PlanId,
    pub selected_option: String,
    pub confidence: Confidence,
    pub candidates_considered: usize,
    pub rationale_summary: String,
    pub policy_evaluation_passed: bool,
    pub made_at: Timestamp,
}

impl Event for DecisionMade {
    fn event_type(&self) -> &'static str {
        "brain.decision.made"
    }
}

/// Published when a decision is rejected by policy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DecisionRejected {
    pub decision_id: DecisionId,
    pub plan_id: PlanId,
    pub violated_rules: Vec<String>, // Blocking rule names
    pub reason: String,
    pub rejected_at: Timestamp,
}

impl Event for DecisionRejected {
    fn event_type(&self) -> &'static str {
        "brain.decision.rejected"
    }
}

/// Published when an unresolvable conflict requires human escalation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EscalationRequired {
    pub decision_id: Option<DecisionId>,
    pub conflict: String,
    pub options: Vec<String>, // Option identifiers
    pub recommended_actor: EscalationChannel,
    pub escalated_at: Timestamp,
}

impl Event for EscalationRequired {
    fn event_type(&self) -> &'static str {
        "brain.decision.escalation.required"
    }
}

/// Channel to which an escalation should be routed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EscalationChannel {
    Operator,
    Human,
    Admin,
    ExternalWebhook,
}
