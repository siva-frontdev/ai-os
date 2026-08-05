//! # AI-OS Capability Policy
//!
//! A thin policy layer that sits between cognition and the runtime manager.
//!
//! Cognition plans against capability ids (`email.send`, …). Before a plan is
//! dispatched to a runtime, LIFE evaluates each capability through this
//! crate to decide:
//!
//! - **risk** — how destructive / irreversible the capability is;
//! - **confirmation** — whether the user must approve the action;
//! - **permissions** — what authority the caller holds; and
//! - **audit** — an append-only record of every dispatch decision.
//!
//! This crate owns no runtime. It never implements [`Runtime`], it only
//! consults a static policy table keyed by capability id and records
//! decisions. The frozen runtime API is untouched.
//!
//! ## Policy before dispatch
//!
//! ```text
//! Planner ──plan──> PlannedAction
//!                       │
//!          CapabilityPolicyRegistry::evaluate(capability, caller)
//!                       │
//!                       ▼
//!              PolicyDecision { allowed, requires_confirmation, ... }
//!                       │ allowed?
//!                  confirm?  yes/no
//!                       │
//!        AuditLog::record(decision)  ──► RuntimeManager::dispatch
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod audit;
pub mod policy;

pub use ai_os_runtime_api::{CapabilityId, RuntimeId};
pub use audit::{AuditEntry, AuditLog, DecisionKind};
pub use policy::{
    CallerPermissions, CapabilityPolicy, Permission, PolicyDecision, PolicyRegistry, RiskLevel,
};
