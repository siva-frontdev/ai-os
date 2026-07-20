//! `CheckpointId` — identifier for a workflow checkpoint.
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CheckpointId(pub Uuid);

impl CheckpointId {
    pub fn new() -> Self { Self(Uuid::new_v4()) }
    pub const fn from_uuid(uuid: Uuid) -> Self { Self(uuid) }
    pub const fn as_uuid(&self) -> &Uuid { &self.0 }
    pub fn into_uuid(self) -> Uuid { self.0 }
}

impl Default for CheckpointId { fn default() -> Self { Self::new() } }
impl fmt::Display for CheckpointId { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { self.0.fmt(f) } }
impl FromStr for CheckpointId { type Err = uuid::Error; fn from_str(s: &str) -> Result<Self, Self::Err> { Uuid::from_str(s).map(Self) } }
impl From<Uuid> for CheckpointId { fn from(uuid: Uuid) -> Self { Self(uuid) } }
impl From<CheckpointId> for Uuid { fn from(id: CheckpointId) -> Self { id.0 } }
