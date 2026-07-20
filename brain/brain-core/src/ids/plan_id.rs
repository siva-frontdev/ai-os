//! `PlanId` — identifier for a plan.
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PlanId(pub Uuid);

impl PlanId {
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

impl Default for PlanId {
    fn default() -> Self {
        Self::new()
    }
}
impl fmt::Display for PlanId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl FromStr for PlanId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::from_str(s).map(Self)
    }
}
impl From<Uuid> for PlanId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}
impl From<PlanId> for Uuid {
    fn from(id: PlanId) -> Self {
        id.0
    }
}
