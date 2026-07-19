//! # Permission Context
//!
//! Role-based authorization for runtime operations.
//!
//! ## Design
//!
//! * **Set-based** — permissions are stored in a `HashSet` for
//!   O(1) lookup.
//! * **Role-based** — a context carries a list of role strings.
//!   Custom implementations can map roles to permissions.
//! * **Fail-closed** — an empty permission set denies
//!   everything.
use std::collections::HashSet;
use std::fmt::Debug;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::error::RuntimeError;

// ── Permission enum ──────────────────────────────────────

/// Atomic permission.
#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub enum Permission {
    Read,
    Write,
    Execute,
    Admin,
    Custom(String),
}

// ── Permission context ───────────────────────────────────

/// Carries the permissions of a session or task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionContext {
    pub session_id: String,
    pub roles: Vec<String>,
    pub permissions: HashSet<Permission>,
}

impl PermissionContext {
    pub fn new(session_id: &str) -> Self {
        Self {
            session_id: session_id.to_string(),
            roles: Vec::new(),
            permissions: HashSet::new(),
        }
    }

    pub fn with_role(mut self, role: &str) -> Self {
        self.roles.push(role.to_string());
        self
    }

    pub fn with_permission(mut self, perm: Permission) -> Self {
        self.permissions.insert(perm);
        self
    }

    pub fn has_permission(&self, perm: &Permission) -> bool {
        self.permissions.contains(perm)
    }

    pub fn has_all(&self, required: &[Permission]) -> bool {
        required.iter().all(|p| self.permissions.contains(p))
    }

    pub fn has_role(&self, role: &str) -> bool {
        self.roles.iter().any(|r| r == role)
    }
}

// ── Permission checker trait ─────────────────────────────

/// Authorisation gate — checks permissions before operations.
pub trait PermissionChecker: Debug + Send + Sync {
    /// Returns `Ok(())` if `ctx` grants every permission in
    /// `required`.
    fn check(&self, ctx: &PermissionContext, required: &[Permission]) -> Result<(), RuntimeError>;
}

// ── Default implementation ───────────────────────────────

#[derive(Debug, Default)]
pub struct DefaultPermissionChecker;

impl DefaultPermissionChecker {
    pub fn new() -> Self {
        Self
    }
}

impl PermissionChecker for DefaultPermissionChecker {
    fn check(&self, ctx: &PermissionContext, required: &[Permission]) -> Result<(), RuntimeError> {
        if ctx.has_all(required) {
            Ok(())
        } else {
            Err(RuntimeError::PermissionDenied(
                "missing required permissions".into(),
            ))
        }
    }
}

// ── Convenience builder ──────────────────────────────────

/// Pre-built permission contexts for common roles.
pub mod roles {
    use super::*;

    pub fn admin(session_id: &str) -> PermissionContext {
        PermissionContext::new(session_id)
            .with_role("admin")
            .with_permission(Permission::Admin)
            .with_permission(Permission::Read)
            .with_permission(Permission::Write)
            .with_permission(Permission::Execute)
    }

    pub fn user(session_id: &str) -> PermissionContext {
        PermissionContext::new(session_id)
            .with_role("user")
            .with_permission(Permission::Read)
            .with_permission(Permission::Write)
    }

    pub fn readonly(session_id: &str) -> PermissionContext {
        PermissionContext::new(session_id)
            .with_role("readonly")
            .with_permission(Permission::Read)
    }
}

// ── Tests ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admin_has_all() {
        let ctx = roles::admin("sess-1");
        assert!(ctx.has_permission(&Permission::Admin));
        assert!(ctx.has_all(&[Permission::Read, Permission::Write]));
    }

    #[test]
    fn user_lacks_admin() {
        let ctx = roles::user("sess-2");
        assert!(!ctx.has_permission(&Permission::Admin));
        assert!(ctx.has_permission(&Permission::Read));
    }

    #[test]
    fn readonly_denies_write() {
        let ctx = roles::readonly("sess-3");
        assert!(ctx.has_permission(&Permission::Read));
        assert!(!ctx.has_permission(&Permission::Write));
    }

    #[test]
    fn default_checker_passes_admin() {
        let checker = DefaultPermissionChecker::new();
        let ctx = roles::admin("sess-4");
        assert!(checker.check(&ctx, &[Permission::Admin, Permission::Read]).is_ok());
    }

    #[test]
    fn default_checker_denies_user_execute() {
        let checker = DefaultPermissionChecker::new();
        let ctx = roles::user("sess-5");
        assert!(checker.check(&ctx, &[Permission::Execute]).is_err());
    }

    #[test]
    fn role_check() {
        let ctx = roles::admin("sess-6");
        assert!(ctx.has_role("admin"));
        assert!(!ctx.has_role("user"));
    }
}
