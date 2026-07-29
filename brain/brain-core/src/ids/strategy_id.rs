use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StrategyId(Uuid);

impl StrategyId {
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

impl Default for StrategyId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for StrategyId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Uuid> for StrategyId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl From<StrategyId> for Uuid {
    fn from(id: StrategyId) -> Self {
        id.0
    }
}
