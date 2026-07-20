use crate::errors::{GoalsError, GoalsResult};
use crate::store::GoalStore;
use crate::types::GoalRecord;
use brain_core::ids::GoalId;
use brain_core::types::GoalPriority;
use brain_core::types::GoalStatus;
use std::collections::HashMap;
use std::sync::RwLock;

#[derive(Debug)]
pub struct InMemoryGoalStore {
    goals: RwLock<HashMap<GoalId, GoalRecord>>,
}

impl InMemoryGoalStore {
    pub fn new() -> Self {
        Self { goals: RwLock::new(HashMap::new()) }
    }
}

impl Default for InMemoryGoalStore {
    fn default() -> Self { Self::new() }
}

#[async_trait::async_trait]
impl GoalStore for InMemoryGoalStore {
    async fn insert(&self, record: GoalRecord) -> GoalsResult<()> {
        let mut goals = self.goals.write().map_err(|e| GoalsError::Internal(e.to_string()))?;
        if goals.contains_key(&record.goal_id) {
            return Err(GoalsError::AlreadyExists(record.goal_id));
        }
        goals.insert(record.goal_id, record);
        Ok(())
    }

    async fn get(&self, goal_id: &GoalId) -> GoalsResult<GoalRecord> {
        let goals = self.goals.read().map_err(|e| GoalsError::Internal(e.to_string()))?;
        goals.get(goal_id).cloned().ok_or(GoalsError::NotFound(*goal_id))
    }

    async fn update(&self, record: GoalRecord) -> GoalsResult<()> {
        let mut goals = self.goals.write().map_err(|e| GoalsError::Internal(e.to_string()))?;
        if !goals.contains_key(&record.goal_id) {
            return Err(GoalsError::NotFound(record.goal_id));
        }
        goals.insert(record.goal_id, record);
        Ok(())
    }

    async fn delete(&self, goal_id: &GoalId) -> GoalsResult<()> {
        let mut goals = self.goals.write().map_err(|e| GoalsError::Internal(e.to_string()))?;
        goals.remove(goal_id).ok_or(GoalsError::NotFound(*goal_id))?;
        Ok(())
    }

    async fn list_by_status(&self, status: GoalStatus) -> GoalsResult<Vec<GoalRecord>> {
        let goals = self.goals.read().map_err(|e| GoalsError::Internal(e.to_string()))?;
        Ok(goals.values().filter(|g| g.status == status).cloned().collect())
    }

    async fn list_by_priority(&self, priority: GoalPriority) -> GoalsResult<Vec<GoalRecord>> {
        let goals = self.goals.read().map_err(|e| GoalsError::Internal(e.to_string()))?;
        Ok(goals.values().filter(|g| g.priority == priority).cloned().collect())
    }

    async fn list_ready(&self) -> GoalsResult<Vec<GoalRecord>> {
        let goals = self.goals.read().map_err(|e| GoalsError::Internal(e.to_string()))?;
        Ok(goals.values().filter(|g| {
            if g.status != GoalStatus::Pending { return false; }
            g.dependencies.iter().all(|dep| {
                goals.get(dep).is_some_and(|dep_goal| dep_goal.status == GoalStatus::Completed)
            })
        }).cloned().collect())
    }

    async fn list_all(&self) -> GoalsResult<Vec<GoalRecord>> {
        let goals = self.goals.read().map_err(|e| GoalsError::Internal(e.to_string()))?;
        Ok(goals.values().cloned().collect())
    }

    async fn exists(&self, goal_id: &GoalId) -> GoalsResult<bool> {
        let goals = self.goals.read().map_err(|e| GoalsError::Internal(e.to_string()))?;
        Ok(goals.contains_key(goal_id))
    }

    async fn count_by_status(&self, status: GoalStatus) -> GoalsResult<usize> {
        let goals = self.goals.read().map_err(|e| GoalsError::Internal(e.to_string()))?;
        Ok(goals.values().filter(|g| g.status == status).count())
    }
}
