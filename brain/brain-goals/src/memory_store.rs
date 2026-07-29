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
    snapshots: RwLock<HashMap<String, Vec<GoalRecord>>>,
}

impl InMemoryGoalStore {
    pub fn new() -> Self {
        Self {
            goals: RwLock::new(HashMap::new()),
            snapshots: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for InMemoryGoalStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl GoalStore for InMemoryGoalStore {
    async fn insert(&self, record: GoalRecord) -> GoalsResult<()> {
        let mut goals = self
            .goals
            .write()
            .map_err(|e| GoalsError::Internal(e.to_string()))?;
        if goals.contains_key(&record.goal_id) {
            return Err(GoalsError::AlreadyExists(record.goal_id));
        }
        goals.insert(record.goal_id, record);
        Ok(())
    }

    async fn get(&self, goal_id: &GoalId) -> GoalsResult<GoalRecord> {
        let goals = self
            .goals
            .read()
            .map_err(|e| GoalsError::Internal(e.to_string()))?;
        goals
            .get(goal_id)
            .cloned()
            .ok_or(GoalsError::NotFound(*goal_id))
    }

    async fn update(&self, record: GoalRecord) -> GoalsResult<()> {
        let mut goals = self
            .goals
            .write()
            .map_err(|e| GoalsError::Internal(e.to_string()))?;
        if !goals.contains_key(&record.goal_id) {
            return Err(GoalsError::NotFound(record.goal_id));
        }
        goals.insert(record.goal_id, record);
        Ok(())
    }

    async fn delete(&self, goal_id: &GoalId) -> GoalsResult<()> {
        let mut goals = self
            .goals
            .write()
            .map_err(|e| GoalsError::Internal(e.to_string()))?;
        goals
            .remove(goal_id)
            .ok_or(GoalsError::NotFound(*goal_id))?;
        Ok(())
    }

    async fn list_by_status(&self, status: GoalStatus) -> GoalsResult<Vec<GoalRecord>> {
        let goals = self
            .goals
            .read()
            .map_err(|e| GoalsError::Internal(e.to_string()))?;
        Ok(goals
            .values()
            .filter(|g| g.status == status)
            .cloned()
            .collect())
    }

    async fn list_by_priority(&self, priority: GoalPriority) -> GoalsResult<Vec<GoalRecord>> {
        let goals = self
            .goals
            .read()
            .map_err(|e| GoalsError::Internal(e.to_string()))?;
        Ok(goals
            .values()
            .filter(|g| g.priority == priority)
            .cloned()
            .collect())
    }

    async fn list_ready(&self) -> GoalsResult<Vec<GoalRecord>> {
        let goals = self
            .goals
            .read()
            .map_err(|e| GoalsError::Internal(e.to_string()))?;
        Ok(goals
            .values()
            .filter(|g| {
                if g.status != GoalStatus::Pending {
                    return false;
                }
                g.dependencies.iter().all(|dep| {
                    goals
                        .get(dep)
                        .is_some_and(|dep_goal| dep_goal.status == GoalStatus::Completed)
                })
            })
            .cloned()
            .collect())
    }

    async fn list_all(&self) -> GoalsResult<Vec<GoalRecord>> {
        let goals = self
            .goals
            .read()
            .map_err(|e| GoalsError::Internal(e.to_string()))?;
        Ok(goals.values().cloned().collect())
    }

    async fn exists(&self, goal_id: &GoalId) -> GoalsResult<bool> {
        let goals = self
            .goals
            .read()
            .map_err(|e| GoalsError::Internal(e.to_string()))?;
        Ok(goals.contains_key(goal_id))
    }

    async fn count_by_status(&self, status: GoalStatus) -> GoalsResult<usize> {
        let goals = self
            .goals
            .read()
            .map_err(|e| GoalsError::Internal(e.to_string()))?;
        Ok(goals.values().filter(|g| g.status == status).count())
    }

    async fn list_active(&self) -> GoalsResult<Vec<GoalRecord>> {
        let goals = self
            .goals
            .read()
            .map_err(|e| GoalsError::Internal(e.to_string()))?;
        Ok(goals
            .values()
            .filter(|g| {
                matches!(
                    g.status,
                    GoalStatus::Active
                        | GoalStatus::Planning
                        | GoalStatus::Executing
                        | GoalStatus::Evaluating
                )
            })
            .cloned()
            .collect())
    }

    async fn search_by_tag(&self, tag: &str) -> GoalsResult<Vec<GoalRecord>> {
        let goals = self
            .goals
            .read()
            .map_err(|e| GoalsError::Internal(e.to_string()))?;
        Ok(goals
            .values()
            .filter(|g| g.tags.iter().any(|t| t == tag))
            .cloned()
            .collect())
    }

    async fn list_since(&self, since: memory_core::Timestamp) -> GoalsResult<Vec<GoalRecord>> {
        let goals = self
            .goals
            .read()
            .map_err(|e| GoalsError::Internal(e.to_string()))?;
        Ok(goals
            .values()
            .filter(|g| g.updated_at >= since)
            .cloned()
            .collect())
    }

    async fn save_snapshot(&self, label: &str) -> GoalsResult<()> {
        let goals = self
            .goals
            .read()
            .map_err(|e| GoalsError::Internal(e.to_string()))?;
        let snapshot: Vec<GoalRecord> = goals.values().cloned().collect();
        let key = format!("__snapshot_{}", label);
        let mut store = self
            .snapshots
            .write()
            .map_err(|e| GoalsError::Internal(e.to_string()))?;
        store.insert(key, snapshot);
        Ok(())
    }

    async fn restore_snapshot(&self, label: &str) -> GoalsResult<()> {
        let key = format!("__snapshot_{}", label);
        let store = self
            .snapshots
            .read()
            .map_err(|e| GoalsError::Internal(e.to_string()))?;
        let snapshot = store
            .get(&key)
            .ok_or_else(|| GoalsError::Internal(format!("snapshot '{}' not found", label)))?;
        let mut goals = self
            .goals
            .write()
            .map_err(|e| GoalsError::Internal(e.to_string()))?;
        goals.clear();
        for record in snapshot {
            goals.insert(record.goal_id, record.clone());
        }
        Ok(())
    }
}
