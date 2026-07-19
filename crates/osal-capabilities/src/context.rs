//! `CapabilityContext` — a subject bound to a set of capabilities.
use crate::CapabilitySet;

/// Associates a subject (process, user, or service) with its capabilities.
///
/// Every security-sensitive operation within the OSAL should consult the
/// context's `CapabilitySet` before proceeding.
#[derive(Debug, Clone)]
pub struct CapabilityContext {
    /// Identifier for the subject (e.g. process PID string, username, or
    /// service name).
    pub subject_id: String,
    /// The set of capabilities granted to this subject.
    pub capabilities: CapabilitySet,
}

impl CapabilityContext {
    /// Create a new context with an empty capability set.
    pub fn new(subject_id: impl Into<String>) -> Self {
        Self {
            subject_id: subject_id.into(),
            capabilities: CapabilitySet::new(),
        }
    }
}
