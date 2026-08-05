//! Shared harness for the runtime validation tests.
//!
//! Exposes the real-binary locator, a killable stdio transport, a scripted
//! intelligence coordinator, and shared dispatch helpers.

pub mod common;

pub use common::binary::mcp_server_bin;
pub use common::coordinator::ScriptedCoordinator;
pub use common::transport::{spawn_mcp_server, TestChildTransport};
pub use common::{
    action_from_planned, dispatch_plan, mcp_runtime_config, runtime_with_transport, temp_root,
    wait_for_observations,
};
