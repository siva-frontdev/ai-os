//! `ToolId` — identifier for a tool candidate.
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ToolId(pub Uuid);

impl ToolId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
    pub const fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }
    pub const fn as_uuid(&self) -> &Uuid {
        &self.0
    }
    pub fn into_uuid(self) -> Uuid {
        self.0
    }
}

impl Default for ToolId {
    fn default() -> Self {
        Self::new()
    }
}
impl fmt::Display for ToolId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl FromStr for ToolId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::from_str(s).map(Self)
    }
}
impl From<Uuid> for ToolId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}
impl From<ToolId> for Uuid {
    fn from(id: ToolId) -> Self {
        id.0
    }
}
