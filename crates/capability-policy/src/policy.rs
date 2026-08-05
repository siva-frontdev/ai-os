//! Policy definitions evaluated before a capability is dispatched.

use std::collections::{HashMap, HashSet};

use ai_os_runtime_api::CapabilityId;
use serde::{Deserialize, Serialize};

/// How risky a capability is. Higher risk → stricter gating.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
pub enum RiskLevel {
    /// Read-only or locally-scoped; safe to execute without review.
    #[default]
    Low,
    /// Modifies state reachable by people; confirm in a shared context.
    Medium,
    /// Sends messages to external people, mutates external state, or
    /// accesses the network in a way the user may not expect.
    High,
    /// Destructive, irreversible, or privileged (filesystem root, tokens,
    /// account changes). Never auto-executed.
    Critical,
}

impl std::fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RiskLevel::Low => write!(f, "low"),
            RiskLevel::Medium => write!(f, "medium"),
            RiskLevel::High => write!(f, "high"),
            RiskLevel::Critical => write!(f, "critical"),
        }
    }
}

/// An authority token the caller holds. Policies reference these by name.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Permission {
    /// Stable key, e.g. `network.outbound`, `filesystem.write`,
    /// `identity.assume`.
    pub authority: String,
    /// Human description of what the authority grants.
    pub description: String,
}

impl Permission {
    /// Build a permission from a short authority key.
    pub fn new(authority: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            authority: authority.into(),
            description: description.into(),
        }
    }
}

/// The policy attached to a single capability id.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CapabilityPolicy {
    /// Static risk classification.
    pub risk: RiskLevel,
    /// Whether the action must be confirmed with the user first.
    pub requires_confirmation: bool,
    /// Minimum permissions a caller must hold to run this capability.
    pub requires: Vec<Permission>,
    /// Whether the action is permanently denied (overrides everything).
    pub denied: bool,
}

impl CapabilityPolicy {
    /// Build a low-risk, permissionless, non-confirmed policy.
    pub fn low() -> Self {
        Self::default()
    }

    /// A policy that always requires user confirmation.
    pub fn confirm(risk: RiskLevel) -> Self {
        Self {
            risk,
            requires_confirmation: true,
            requires: Vec::new(),
            denied: false,
        }
    }

    /// A policy that is permanently denied.
    pub fn denied() -> Self {
        Self {
            risk: RiskLevel::Critical,
            requires_confirmation: true,
            requires: Vec::new(),
            denied: true,
        }
    }
}

/// Set of permissions the caller currently holds.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CallerPermissions(pub HashSet<String>);

impl CallerPermissions {
    /// An empty permission set (the conservative default).
    pub fn empty() -> Self {
        Self(HashSet::new())
    }

    /// Load a permission set from an authority list.
    pub fn from_authorities(authorities: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self(authorities.into_iter().map(Into::into).collect())
    }

    /// Whether the caller holds `authority`.
    pub fn allows(&self, authority: &str) -> bool {
        self.0.contains(authority)
    }

    /// Whether the caller holds every `authority` in `policy.requires`.
    pub fn satisfies(&self, requires: &[Permission]) -> bool {
        requires.iter().all(|p| self.0.contains(&p.authority))
    }
}

/// The outcome of evaluating a capability against its policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyDecision {
    /// The capability that was evaluated.
    pub capability: CapabilityId,
    /// How risky the capability is.
    pub risk: RiskLevel,
    /// Whether the caller must confirm before execution.
    pub requires_confirmation: bool,
    /// Whether the caller is allowed to execute the capability at all.
    pub allowed: bool,
    /// Why the capability was denied, when `allowed` is false.
    pub denial_reason: Option<String>,
}

impl PolicyDecision {
    /// Whether the action can be dispatched immediately.
    pub fn can_execute(&self) -> bool {
        self.allowed && !self.requires_confirmation
    }
}

/// A static policy table keyed by capability id.
///
/// The registry is immutable after construction; it is the single source of
/// truth for "what each capability requires". Cognition never edits it.
#[derive(Debug, Clone, Default)]
pub struct PolicyRegistry {
    policies: HashMap<CapabilityId, CapabilityPolicy>,
}

impl PolicyRegistry {
    /// An empty registry (all capabilities fall back to [`CapabilityPolicy::none`]).
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a policy for a capability id.
    pub fn register(&mut self, id: impl Into<CapabilityId>, policy: CapabilityPolicy) -> &mut Self {
        self.policies.insert(id.into(), policy);
        self
    }

    /// The policy for a capability, or a safe default when unspecified.
    pub fn policy_for(&self, id: &CapabilityId) -> &CapabilityPolicy {
        self.policies.get(id).unwrap_or(&DEFAULT_POLICY)
    }

    /// Evaluate a capability for a given caller, producing a decision.
    pub fn evaluate(
        &self,
        capability: &CapabilityId,
        caller: &CallerPermissions,
    ) -> PolicyDecision {
        let policy = self.policy_for(capability);

        if policy.denied {
            return PolicyDecision {
                capability: capability.clone(),
                risk: policy.risk,
                requires_confirmation: false,
                allowed: false,
                denial_reason: Some("capability is globally denied".into()),
            };
        }

        if !caller.satisfies(&policy.requires) {
            let missing: Vec<String> = policy
                .requires
                .iter()
                .filter(|p| !caller.allows(&p.authority))
                .map(|p| p.authority.clone())
                .collect();
            return PolicyDecision {
                capability: capability.clone(),
                risk: policy.risk,
                requires_confirmation: policy.requires_confirmation,
                allowed: false,
                denial_reason: Some(format!("missing permissions: {}", missing.join(", "))),
            };
        }

        PolicyDecision {
            capability: capability.clone(),
            risk: policy.risk,
            requires_confirmation: policy.requires_confirmation,
            allowed: true,
            denial_reason: None,
        }
    }
}

/// The shared default policy used when a capability is unregistered.
static DEFAULT_POLICY: CapabilityPolicy = CapabilityPolicy {
    risk: RiskLevel::Low,
    requires_confirmation: false,
    requires: Vec::new(),
    denied: false,
};

#[cfg(test)]
mod tests {
    use super::*;

    fn id(s: &str) -> CapabilityId {
        CapabilityId::new(s)
    }

    #[test]
    fn unregistered_capability_defaults_to_low_risk_no_confirm() {
        let registry = PolicyRegistry::new();
        let caller = CallerPermissions::empty();
        let decision = registry.evaluate(&id("unknown.do_thing"), &caller);
        assert!(decision.allowed);
        assert!(!decision.requires_confirmation);
        assert_eq!(decision.risk, RiskLevel::Low);
        assert!(decision.can_execute());
    }

    #[test]
    fn critical_capability_requires_confirmation_and_permission() {
        let mut registry = PolicyRegistry::new();
        registry.register(
            "filesystem.write_root",
            CapabilityPolicy {
                risk: RiskLevel::Critical,
                requires_confirmation: true,
                requires: vec![Permission::new(
                    "filesystem.root_write",
                    "write outside sandbox",
                )],
                denied: false,
            },
        );

        // Caller lacks the permission: denied.
        let caller = CallerPermissions::empty();
        let decision = registry.evaluate(&id("filesystem.write_root"), &caller);
        assert!(!decision.allowed);
        assert!(decision
            .denial_reason
            .as_deref()
            .unwrap()
            .contains("missing permissions"));

        // Caller has the permission: allowed but must confirm.
        let caller = CallerPermissions::from_authorities(["filesystem.root_write"]);
        let decision = registry.evaluate(&id("filesystem.write_root"), &caller);
        assert!(decision.allowed);
        assert!(decision.requires_confirmation);
        assert!(!decision.can_execute());
    }

    #[test]
    fn denied_capability_is_blocked_even_with_permissions() {
        let mut registry = PolicyRegistry::new();
        registry.register(
            "identity.delete_account",
            CapabilityPolicy {
                risk: RiskLevel::Critical,
                requires_confirmation: true,
                requires: vec![Permission::new("identity.assume", "assume identity")],
                denied: true,
            },
        );

        let caller = CallerPermissions::from_authorities(["identity.assume"]);
        let decision = registry.evaluate(&id("identity.delete_account"), &caller);
        assert!(!decision.allowed);
        assert_eq!(
            decision.denial_reason.as_deref(),
            Some("capability is globally denied")
        );
    }

    #[test]
    fn medium_risk_low_permission_is_immediately_executable() {
        let mut registry = PolicyRegistry::new();
        registry.register(
            "email.send",
            CapabilityPolicy {
                risk: RiskLevel::Medium,
                requires_confirmation: false,
                requires: vec![Permission::new("network.outbound", "send outbound mail")],
                denied: false,
            },
        );

        // No permission → denied.
        let caller = CallerPermissions::empty();
        let decision = registry.evaluate(&id("email.send"), &caller);
        assert!(!decision.allowed);

        // With permission and no confirmation → executable immediately.
        let caller = CallerPermissions::from_authorities(["network.outbound"]);
        let decision = registry.evaluate(&id("email.send"), &caller);
        assert!(decision.allowed);
        assert!(!decision.requires_confirmation);
        assert!(decision.can_execute());
    }

    #[test]
    fn risk_levels_order() {
        assert!(RiskLevel::Low < RiskLevel::Medium);
        assert!(RiskLevel::Medium < RiskLevel::High);
        assert!(RiskLevel::High < RiskLevel::Critical);
    }

    #[test]
    fn caller_permissions_satisfy_subset() {
        let caller = CallerPermissions::from_authorities(["network.outbound", "extra"]);
        let requires = vec![Permission::new("network.outbound", "send")];
        assert!(caller.satisfies(&requires));
        let requires = vec![
            Permission::new("network.outbound", "send"),
            Permission::new("admin", "admin"),
        ];
        assert!(!caller.satisfies(&requires));
    }
}
