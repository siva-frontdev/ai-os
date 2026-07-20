//! `ThoughtId` — identifier for a reasoning session / thought trace.
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

/// Opaque identifier for a reasoning session (thought trace).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ThoughtId(pub Uuid);

impl ThoughtId {
    pub fn new() -> Self { Self(Uuid::new_v4()) }
    pub const fn from_uuid(uuid: Uuid) -> Self { Self(uuid) }
    pub const fn as_uuid(&self) -> &Uuid { &self.0 }
    pub fn into_uuid(self) -> Uuid { self.0 }
}

impl Default for ThoughtId { fn default() -> Self { Self::new() } }
impl fmt::Display for ThoughtId { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { self.0.fmt(f) } }
impl FromStr for ThoughtId { type Err = uuid::Error; fn from_str(s: &str) -> Result<Self, Self::Err> { Uuid::from_str(s).map(Self) } }
impl From<Uuid> for ThoughtId { fn from(uuid: Uuid) -> Self { Self(uuid) } }
impl From<ThoughtId> for Uuid { fn from(id: ThoughtId) -> Self { id.0 } }
