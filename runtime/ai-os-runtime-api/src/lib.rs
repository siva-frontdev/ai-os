//! # AI-OS Runtime API
//!
//! A vendor-neutral abstraction for execution runtimes.
//!
//! The Runtime API is the contract between AI-OS cognition (Planner,
//! Cognitive Loop, Memory) and external execution runtimes. Cognition
//! addresses **capabilities** (`email.send`, `telegram.post_message`),
//! never a vendor. A runtime — OpenClaw, a native runtime, or any future
//! runtime — implements the [`Runtime`] trait and becomes replaceable
//! without touching anything under `brain/`.
//!
//! ## Principles
//!
//! 1. **Capability-addressed, not vendor-addressed.** The Planner plans
//!    against namespaced capability ids. The runtime maps a capability to
//!    a concrete implementation.
//! 2. **Bidirectional I/O.** The runtime executes actions (outbound) and
//!    produces observations (inbound). Both directions are first-class.
//! 3. **Structured payloads.** Action parameters and results are typed
//!    JSON values validated against declared schemas — never stringly-typed
//!    shell fragments.
//! 4. **Replaceable.** The trait is used behind `Arc<dyn Runtime>`.
//!    Swapping runtimes requires zero changes in the cognitive core.
//! 5. **Fail-safe.** Errors are typed, never panics. Unknown capabilities
//!    are rejected before execution with `ActionStatus::Failed`.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod action;
pub mod capability;
pub mod default_runtime;
pub mod dispatcher;
pub mod error;
pub mod observation;
pub mod runtime;
pub mod schema;
pub mod time;

pub use action::{Action, ActionResult, ActionStatus, RuntimeErrorPayload};
pub use capability::{Capability, CapabilityId, SideEffect};
pub use default_runtime::DefaultRuntime;
pub use dispatcher::RuntimeDispatcher;
pub use error::RuntimeError;
pub use observation::{Observation, ObservationKind};
pub use runtime::{Runtime, RuntimeEvent, RuntimeHealth, RuntimeId};
pub use schema::validate;
pub use time::now_ms;
