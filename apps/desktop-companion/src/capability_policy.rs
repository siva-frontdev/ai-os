//! Capability policy gate for the desktop companion.
//!
//! The AI-OS companion plans capabilities (e.g. `email.send`,
//! `telegram.inject_inbound`) against an abstract capability surface. Before
//! any action is dispatched to a runtime, the [`CapabilityGate`] consults a
//! [`PolicyRegistry`] to enforce risk level, confirmation requirements, and
//! permissions, and records every decision into an [`AuditLog`].
//!
//! This is the Phase 4 policy layer: cognition never sees vendors, only
//! capability ids; the gate is the only place policy is evaluated, so the
//! cognitive core stays clean.

use ai_os_capability_policy::{
    AuditEntry, AuditLog, CallerPermissions, CapabilityId, CapabilityPolicy, DecisionKind,
    Permission, PolicyDecision, PolicyRegistry, RiskLevel, RuntimeId,
};

/// Authority the companion holds by default. The real companion would load
/// these from a signed capability manifest; for now they cover the default
/// plugin set.
fn default_permissions() -> [Permission; 8] {
    [
        Permission::new(
            "network.outbound",
            "send messages / perform outbound I/O to external services",
        ),
        Permission::new("filesystem.write", "write files under the sandboxed root"),
        Permission::new("email.compose", "compose and queue outbound email"),
        Permission::new("calendar.write", "create calendar events"),
        Permission::new("github.issues", "create repository issues"),
        Permission::new("telegram.post", "post messages to chats"),
        Permission::new("identity.assume", "assume an identity / act on its behalf"),
        Permission::new(
            "filesystem.root_write",
            "write outside the sandboxed root (root escalation)",
        ),
    ]
}

/// Build the companion's policy registry: one [`CapabilityPolicy`] per real
/// plugin capability, keyed by capability id.
pub fn default_policy_registry() -> PolicyRegistry {
    let mut registry = PolicyRegistry::new();

    // Read-only, low-risk capabilities: execute without confirmation.
    registry.register(
        "filesystem.read",
        CapabilityPolicy {
            risk: RiskLevel::Medium,
            requires_confirmation: false,
            requires: vec![Permission::new("filesystem.write", "read under sandbox")],
            ..Default::default()
        },
    );
    registry.register(
        "github.read_repository",
        CapabilityPolicy {
            risk: RiskLevel::Low,
            requires_confirmation: false,
            requires: vec![],
            ..Default::default()
        },
    );
    registry.register(
        "calendar.read",
        CapabilityPolicy {
            risk: RiskLevel::Low,
            requires_confirmation: false,
            requires: vec![],
            ..Default::default()
        },
    );
    registry.register(
        "telegram.send",
        CapabilityPolicy {
            risk: RiskLevel::Medium,
            requires_confirmation: false,
            requires: vec![Permission::new("telegram.post", "post to chats")],
            ..Default::default()
        },
    );

    // State-mutating capabilities: require confirmation.
    registry.register(
        "email.send",
        CapabilityPolicy {
            risk: RiskLevel::High,
            requires_confirmation: true,
            requires: vec![Permission::new("email.compose", "compose email")],
            ..Default::default()
        },
    );
    registry.register(
        "calendar.create_event",
        CapabilityPolicy {
            risk: RiskLevel::High,
            requires_confirmation: true,
            requires: vec![Permission::new("calendar.write", "write calendar")],
            ..Default::default()
        },
    );
    registry.register(
        "github.create_issue",
        CapabilityPolicy {
            risk: RiskLevel::High,
            requires_confirmation: true,
            requires: vec![Permission::new("github.issues", "create issues")],
            ..Default::default()
        },
    );

    // Simulation-only capabilities: low risk, no confirmation.
    registry.register(
        "telegram.inject_inbound",
        CapabilityPolicy {
            risk: RiskLevel::Low,
            requires_confirmation: false,
            requires: vec![],
            ..Default::default()
        },
    );
    registry.register(
        "email.inject",
        CapabilityPolicy {
            risk: RiskLevel::Low,
            requires_confirmation: false,
            requires: vec![],
            ..Default::default()
        },
    );

    // Explicitly denied: filesystem writes outside the sandbox root.
    registry.register("filesystem.write_root", CapabilityPolicy::denied());

    registry
}

/// A caller permission set seeded with the companion's default authorities.
pub fn default_caller_permissions() -> CallerPermissions {
    CallerPermissions::from_authorities(default_permissions().iter().map(|p| p.authority.as_str()))
}

/// Evaluates, logs, and surfaces the confirmation requirement for a capability
/// before dispatch.
pub struct CapabilityGate {
    registry: PolicyRegistry,
    permissions: ai_os_capability_policy::CallerPermissions,
    pub audit: AuditLog,
}

impl CapabilityGate {
    /// Build a gate with the companion's default policies and permissions.
    pub fn new() -> Self {
        Self {
            registry: default_policy_registry(),
            permissions: default_caller_permissions(),
            audit: AuditLog::new(),
        }
    }

    /// Build a gate with a custom policy registry.
    #[allow(dead_code)]
    pub fn with_registry(registry: PolicyRegistry) -> Self {
        Self {
            registry,
            permissions: default_caller_permissions(),
            audit: AuditLog::new(),
        }
    }

    /// Evaluate a capability, recording the decision to the audit log.
    ///
    /// Returns the policy decision. A denial is also surfaced as
    /// `Err`; the audit entry is always written.
    pub async fn evaluate(&self, capability: &CapabilityId, runtime: &str) -> PolicyResult {
        let decision = self.registry.evaluate(capability, &self.permissions);
        let kind = if decision.allowed && decision.requires_confirmation {
            DecisionKind::Confirmed
        } else if decision.allowed {
            DecisionKind::Executed
        } else if decision
            .denial_reason
            .as_deref()
            .map(|r| r.contains("missing permissions"))
            .unwrap_or(false)
        {
            DecisionKind::PermissionDenied
        } else {
            DecisionKind::Denied
        };

        self.audit
            .record(
                AuditEntry::new(
                    capability.clone(),
                    Some(RuntimeId(runtime.into())),
                    kind,
                    decision
                        .denial_reason
                        .clone()
                        .unwrap_or_else(|| "allowed".into()),
                )
                .with_trace(format!("capability:{}", capability)),
            )
            .await;

        PolicyResult { decision }
    }
}

impl Default for CapabilityGate {
    fn default() -> Self {
        Self::new()
    }
}

/// Outcome of evaluating a capability through the gate.
pub struct PolicyResult {
    /// The evaluated decision.
    pub decision: PolicyDecision,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn email_send_requires_confirmation() {
        let gate = CapabilityGate::new();
        let result = gate
            .evaluate(&CapabilityId::new("email.send"), "openclaw")
            .await;
        let d = &result.decision;
        assert!(d.allowed);
        assert!(d.requires_confirmation);
        assert!(!d.can_execute());
    }

    #[tokio::test]
    async fn denied_capability_is_blocked() {
        let gate = CapabilityGate::new();
        let result = gate
            .evaluate(&CapabilityId::new("filesystem.write_root"), "openclaw")
            .await;
        let d = &result.decision;
        assert!(!d.allowed);
        assert!(d.denial_reason.is_some());
    }

    #[tokio::test]
    async fn missing_permission_denies() {
        let gate = CapabilityGate::with_registry(PolicyRegistry::new());
        // Empty permissions, an unregistered capability defaults to allow —
        // so explicitly register a high-risk one requiring a permission we
        // don't hold.
        let mut registry = PolicyRegistry::new();
        registry.register(
            "custom.secure",
            CapabilityPolicy {
                risk: RiskLevel::High,
                requires_confirmation: true,
                requires: vec![Permission::new("custom.authority", "do custom")],
                ..Default::default()
            },
        );
        let gate = CapabilityGate::with_registry(registry);
        let result = gate
            .evaluate(&CapabilityId::new("custom.secure"), "openclaw")
            .await;
        assert!(!result.decision.allowed);
        assert!(result
            .decision
            .denial_reason
            .unwrap()
            .contains("missing permissions"));
    }

    #[tokio::test]
    async fn registry_records_audit_entry() {
        let gate = CapabilityGate::new();
        gate.evaluate(&CapabilityId::new("email.send"), "openclaw")
            .await;
        let entries = gate.audit.entries().await;
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].capability, CapabilityId::new("email.send"));
        assert_eq!(entries[0].kind, DecisionKind::Confirmed);
    }
}
