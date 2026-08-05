//! Append-only audit log of capability dispatch decisions.

use std::sync::Arc;

use ai_os_runtime_api::{CapabilityId, RuntimeId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// What kind of disposition the audit captured.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DecisionKind {
    /// Executed without confirmation.
    Executed,
    /// Required user confirmation before execution.
    Confirmed,
    /// Denied by policy.
    Denied,
    /// Rejected because of a missing permission.
    PermissionDenied,
}

/// A single audited decision about a capability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    /// When the decision was made (UTC).
    pub at: DateTime<Utc>,
    /// The capability addressed.
    pub capability: CapabilityId,
    /// Which runtime was involved (if known).
    pub runtime: Option<RuntimeId>,
    /// How the decision discharged.
    pub kind: DecisionKind,
    /// Human-readable explanation (reason for denial, etc.).
    pub reason: String,
    /// Correlation id of the originating plan/action, if any.
    pub trace_id: Option<String>,
}

impl AuditEntry {
    /// Build and timestamp a new entry.
    pub fn new(
        capability: CapabilityId,
        runtime: Option<RuntimeId>,
        kind: DecisionKind,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            at: Utc::now(),
            capability,
            runtime,
            kind,
            reason: reason.into(),
            trace_id: None,
        }
    }

    /// Attach a trace id (for correlation with a Cognitive Loop cycle).
    pub fn with_trace(mut self, trace_id: impl Into<String>) -> Self {
        self.trace_id = Some(trace_id.into());
        self
    }
}

/// An in-memory append-only audit log.
///
/// The log is shared by cloning the [`Arc`]; entries are appended behind a
/// `tokio::sync` mutex so the cognitive worker and runtime dispatcher can
/// record concurrently.
#[derive(Debug, Clone, Default)]
pub struct AuditLog {
    entries: Arc<tokio::sync::Mutex<Vec<AuditEntry>>>,
}

impl AuditLog {
    /// A new empty audit log.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a decision.
    pub async fn record(&self, entry: AuditEntry) {
        self.entries.lock().await.push(entry);
    }

    /// A snapshot of every recorded decision, in append order.
    pub async fn entries(&self) -> Vec<AuditEntry> {
        self.entries.lock().await.clone()
    }

    /// Number of recorded decisions.
    pub async fn len(&self) -> usize {
        self.entries.lock().await.len()
    }

    /// Whether the log is empty.
    pub async fn is_empty(&self) -> bool {
        self.entries.lock().await.is_empty()
    }

    /// Count decisions by kind.
    pub async fn count_by(&self, kind: DecisionKind) -> usize {
        self.entries
            .lock()
            .await
            .iter()
            .filter(|e| e.kind == kind)
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn records_and_retrieves_entries_in_order() {
        let log = AuditLog::new();
        assert!(log.is_empty().await);

        log.record(
            AuditEntry::new(
                CapabilityId::new("email.send"),
                Some(RuntimeId("openclaw".into())),
                DecisionKind::Executed,
                "dispatched",
            )
            .with_trace("trace-1"),
        )
        .await;
        log.record(
            AuditEntry::new(
                CapabilityId::new("filesystem.write_root"),
                Some(RuntimeId("openclaw".into())),
                DecisionKind::PermissionDenied,
                "missing permissions: filesystem.root_write",
            )
            .with_trace("trace-2"),
        )
        .await;

        assert_eq!(log.len().await, 2);
        let entries = log.entries().await;
        assert_eq!(entries[0].capability, CapabilityId::new("email.send"));
        assert_eq!(entries[1].kind, DecisionKind::PermissionDenied);
        assert_eq!(entries[0].trace_id.as_deref(), Some("trace-1"));
    }

    #[tokio::test]
    async fn count_by_kind() {
        let log = AuditLog::new();
        log.record(AuditEntry::new(
            CapabilityId::new("a"),
            None,
            DecisionKind::Executed,
            "ok",
        ))
        .await;
        log.record(AuditEntry::new(
            CapabilityId::new("b"),
            None,
            DecisionKind::Denied,
            "no",
        ))
        .await;

        assert_eq!(log.count_by(DecisionKind::Executed).await, 1);
        assert_eq!(log.count_by(DecisionKind::Denied).await, 1);
        assert_eq!(log.count_by(DecisionKind::Confirmed).await, 0);
    }
}
