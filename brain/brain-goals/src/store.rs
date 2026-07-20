use crate::errors::GoalsResult;
use crate::types::GoalRecord;
use brain_core::ids::GoalId;
use brain_core::types::GoalStatus;

#[async_trait::async_trait]
pub trait GoalStore: Send + Sync {
    async fn insert(&self, record: GoalRecord) -> GoalsResult<()>;
    async fn get(&self, goal_id: &GoalId) -> GoalsResult<GoalRecord>;
    async fn update(&self, record: GoalRecord) -> GoalsResult<()>;
    async fn delete(&self, goal_id: &GoalId) -> GoalsResult<()>;
    async fn list_by_status(&self, status: GoalStatus) -> GoalsResult<Vec<GoalRecord>>;
    async fn list_by_priority(&self, priority: brain_core::types::GoalPriority) -> GoalsResult<Vec<GoalRecord>>;
    async fn list_ready(&self) -> GoalsResult<Vec<GoalRecord>>;
    async fn list_all(&self) -> GoalsResult<Vec<GoalRecord>>;
    async fn exists(&self, goal_id: &GoalId) -> GoalsResult<bool>;
    async fn count_by_status(&self, status: GoalStatus) -> GoalsResult<usize>;
}
