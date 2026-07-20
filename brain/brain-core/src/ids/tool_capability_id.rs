//! `ToolCapabilityId` — identifier for a tool capability (not a specific tool).
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

/// Opaque identifier for a tool capability.
///
/// A `ToolCapabilityId` identifies *what* a tool can do, not *which*
/// tool provides it.
/// Examples: `"read_file"`, `"run_command"`, `"search_web"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ToolCapabilityId(pub Uuid);

impl ToolCapabilityId {
    pub fn new() -> Self { Self(Uuid::new_v4()) }
    pub const fn from_uuid(uuid: Uuid) -> Self { Self(uuid) }
    pub const fn as_uuid(&self) -> &Uuid { &self.0 }
    pub fn into_uuid(self) -> Uuid { self.0 }
}

impl Default for ToolCapabilityId { fn default() -> Self { Self::new() } }
impl fmt::Display for ToolCapabilityId { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { self.0.fmt(f) } }
impl FromStr for ToolCapabilityId { type Err = uuid::Error; fn from_str(s: &str) -> Result<Self, Self::Err> { Uuid::from_str(s).map(Self) } }
impl From<Uuid> for ToolCapabilityId { fn from(uuid: Uuid) -> Self { Self(uuid) } }
impl From<ToolCapabilityId> for Uuid { fn from(id: ToolCapabilityId) -> Self { id.0 } }
