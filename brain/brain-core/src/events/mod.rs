//! Event definitions for the Brain Platform.
//!
//! Every Brain event struct implements [`ai_os_core::events::Event`].
//! Events are published via the Core `EventBus` and identified by
//! their `event_type()` string.
//!
//! ## Event Categories
//!
//! | Category | Module |
//! |---|---|
//! | Goal lifecycle | [`goal`] |
//! | Planning | [`plan`] |
//! | Reasoning | [`reasoning`] |
//! | Decision | [`decision`] |
//! | Reflection | [`reflection`] |
//! | Learning | [`learning`] |
//! | Workflow | [`workflow`] |
//! | Coordinator / lifecycle | [`coordinator`] |
//! | Policy | [`policy`] |

pub mod cognition;
pub mod coordinator;
pub mod decision;
pub mod goal;
pub mod learning;
pub mod model;
pub mod plan;
pub mod policy;
pub mod reasoning;
pub mod reflection;
pub mod workflow;
