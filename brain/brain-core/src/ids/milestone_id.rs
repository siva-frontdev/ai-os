use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MilestoneId(Uuid);

impl MilestoneId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }
    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
    pub fn into_uuid(self) -> Uuid {
        self.0
    }
}

impl Default for MilestoneId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for MilestoneId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Uuid> for MilestoneId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl From<MilestoneId> for Uuid {
    fn from(id: MilestoneId) -> Self {
        id.0
    }
}
