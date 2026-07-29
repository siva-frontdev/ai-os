use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ObjectiveId(Uuid);

impl ObjectiveId {
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

impl Default for ObjectiveId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for ObjectiveId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Uuid> for ObjectiveId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl From<ObjectiveId> for Uuid {
    fn from(id: ObjectiveId) -> Self {
        id.0
    }
}
