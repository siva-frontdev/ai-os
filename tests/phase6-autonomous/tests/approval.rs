//! Phase 6 — Autonomous Agent: approval gating via the capability policy.
//!
//! Proves every dispatched capability is evaluated against a policy table
//! before execution: default deny for privileged actions, required
//! confirmation for risky ones, and explicit grants for trusted callers.

use ai_os_capability_policy::{
    CallerPermissions, CapabilityPolicy, Permission, PolicyRegistry, RiskLevel,
};
use ai_os_runtime_api::CapabilityId;

fn id(s: &str) -> CapabilityId {
    CapabilityId::new(s)
}

/// Build a registry mirroring the platform's capability risk table.
fn platform_registry() -> PolicyRegistry {
    let mut registry = PolicyRegistry::new();
    registry
        .register("filesystem.read", CapabilityPolicy::low())
        .register("email.send", CapabilityPolicy::confirm(RiskLevel::High))
        .register(
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
        )
        .register(
            "system.shutdown",
            CapabilityPolicy {
                risk: RiskLevel::Critical,
                requires_confirmation: true,
                requires: vec![Permission::new("system.admin", "admin access")],
                denied: false,
            },
        )
        .register("identity.assume", CapabilityPolicy::denied());
    registry
}

#[test]
fn low_risk_capability_auto_executes_without_confirmation() {
    let registry = platform_registry();
    let caller = CallerPermissions::empty();

    let decision = registry.evaluate(&id("filesystem.read"), &caller);
    assert!(decision.allowed);
    assert!(!decision.requires_confirmation);
    assert!(decision.can_execute(), "low risk dispatches immediately");
}

#[test]
fn high_risk_capability_requires_confirmation() {
    let registry = platform_registry();
    let caller = CallerPermissions::empty();

    let decision = registry.evaluate(&id("email.send"), &caller);
    assert!(decision.allowed);
    assert!(
        decision.requires_confirmation,
        "sending external messages requires user confirmation"
    );
    assert!(
        !decision.can_execute(),
        "must not auto-execute without approval"
    );
}

#[test]
fn privileged_capability_denied_without_permission() {
    let registry = platform_registry();
    let caller = CallerPermissions::empty();

    let decision = registry.evaluate(&id("filesystem.write_root"), &caller);
    assert!(!decision.allowed);
    assert!(decision.denial_reason.is_some(), "denial carries a reason");
    assert!(!decision.can_execute());
}

#[test]
fn privileged_capability_allowed_with_granted_permission() {
    let registry = platform_registry();
    let caller = CallerPermissions::from_authorities(["filesystem.root_write"]);

    let decision = registry.evaluate(&id("filesystem.write_root"), &caller);
    assert!(decision.allowed);
    assert!(
        decision.requires_confirmation,
        "still needs a human in the loop"
    );
    assert!(!decision.can_execute());
}

#[test]
fn denied_capability_cannot_be_executed_by_anyone() {
    let registry = platform_registry();
    let admin = CallerPermissions::from_authorities(["system.admin", "filesystem.root_write"]);

    let decision = registry.evaluate(&id("identity.assume"), &admin);
    assert!(
        !decision.allowed,
        "globally denied capability is never allowed"
    );
    assert!(!decision.can_execute());
}

#[test]
fn risk_levels_are_ordered_and_reflected() {
    use std::cmp::Ordering;
    assert!(RiskLevel::Low < RiskLevel::Medium);
    assert!(RiskLevel::Medium < RiskLevel::High);
    assert!(RiskLevel::High < RiskLevel::Critical);
    assert_eq!(RiskLevel::Low.cmp(&RiskLevel::Low), Ordering::Equal);
}

#[test]
fn unregistered_capability_falls_back_to_safe_default() {
    let registry = PolicyRegistry::new();
    let caller = CallerPermissions::empty();

    let decision = registry.evaluate(&id("unknown.capability"), &caller);
    assert!(decision.allowed);
    assert_eq!(decision.risk, RiskLevel::Low);
    assert!(decision.can_execute());
}

#[test]
fn caller_permission_set_satisfies_policy_requirements() {
    let policy = CapabilityPolicy {
        risk: RiskLevel::High,
        requires_confirmation: false,
        requires: vec![
            Permission::new("network.outbound", "outbound network"),
            Permission::new("secrets.read", "read secrets"),
        ],
        denied: false,
    };

    let partial = CallerPermissions::from_authorities(["network.outbound"]);
    assert!(
        !partial.satisfies(&policy.requires),
        "partial grant insufficient"
    );

    let full = CallerPermissions::from_authorities(["network.outbound", "secrets.read"]);
    assert!(
        full.satisfies(&policy.requires),
        "full grant satisfies policy"
    );
}
