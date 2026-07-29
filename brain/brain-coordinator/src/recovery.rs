use brain_core::ids::GoalId;
use brain_core::types::GoalStatus;
use brain_goals::types::GoalRecord;
use brain_goals::{GoalManager, GoalStore};

use crate::errors::{CoordinatorError, CoordinatorResult};

/// Handles automatic recovery and resumption of interrupted goals.
pub struct RecoveryManager;

impl RecoveryManager {
    pub fn new() -> Self {
        Self
    }

    /// Find goals that were interrupted and need resumption.
    ///
    /// Interrupted goals are those still in `Active`, `Planning`,
    /// `Executing`, `Evaluating`, or `Recovering` status that were
    /// left behind after a restart.
    pub async fn find_interrupted_goals(
        store: &dyn GoalStore,
    ) -> CoordinatorResult<Vec<GoalRecord>> {
        let all = store
            .list_all()
            .await
            .map_err(|e| CoordinatorError::Goal(e.to_string()))?;
        Ok(all
            .into_iter()
            .filter(|g| {
                matches!(
                    g.status,
                    GoalStatus::Active
                        | GoalStatus::Planning
                        | GoalStatus::Executing
                        | GoalStatus::Evaluating
                        | GoalStatus::Recovering
                )
            })
            .collect())
    }

    /// Find goals that were paused or blocked at interruption.
    pub async fn find_paused_goals(store: &dyn GoalStore) -> CoordinatorResult<Vec<GoalRecord>> {
        let all = store
            .list_all()
            .await
            .map_err(|e| CoordinatorError::Goal(e.to_string()))?;
        Ok(all
            .into_iter()
            .filter(|g| matches!(g.status, GoalStatus::Paused | GoalStatus::Blocked))
            .collect())
    }

    /// Automatically resume all interrupted goals.
    ///
    /// Returns the list of goals that were successfully resumed.
    pub async fn resume_all(manager: &GoalManager) -> CoordinatorResult<Vec<GoalId>> {
        let store = manager.store();
        let interrupted = Self::find_interrupted_goals(&**store).await?;
        let paused = Self::find_paused_goals(&**store).await?;

        let mut resumed = Vec::new();

        for goal in interrupted {
            match manager.recover_goal(&goal.goal_id).await {
                Ok(r) => {
                    resumed.push(r.goal_id);
                }
                Err(e) => {
                    eprintln!("[recovery] failed to resume goal {}: {e}", goal.goal_id);
                }
            }
        }

        // Paused/blocked goals stay paused; just note them
        for goal in paused {
            resumed.push(goal.goal_id);
        }

        Ok(resumed)
    }

    /// Verify goal persistence by round-tripping through snapshot/restore.
    pub async fn verify_persistence(manager: &GoalManager) -> CoordinatorResult<bool> {
        let store = manager.store();
        let all = store
            .list_all()
            .await
            .map_err(|e| CoordinatorError::Goal(e.to_string()))?;
        let count = all.len();

        // Snapshot current state
        manager
            .snapshot("persistence_verify")
            .await
            .map_err(|e| CoordinatorError::Goal(e.to_string()))?;

        // Restore from snapshot
        manager
            .restore("persistence_verify")
            .await
            .map_err(|e| CoordinatorError::Goal(e.to_string()))?;

        let after = store
            .list_all()
            .await
            .map_err(|e| CoordinatorError::Goal(e.to_string()))?;

        Ok(after.len() == count)
    }
}

impl Default for RecoveryManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use brain_goals::memory_store::InMemoryGoalStore;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_recovery_completes_with_no_goals() {
        let store = InMemoryGoalStore::new();
        let manager = GoalManager::new(Arc::new(store));
        let result = RecoveryManager::resume_all(&manager).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_verify_persistence_empty_store() {
        let store = InMemoryGoalStore::new();
        let manager = GoalManager::new(Arc::new(store));
        let result = RecoveryManager::verify_persistence(&manager).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), true);
    }

    #[tokio::test]
    async fn test_find_interrupted_goals_empty() {
        let store = InMemoryGoalStore::new();
        let result = RecoveryManager::find_interrupted_goals(&store).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }
}
