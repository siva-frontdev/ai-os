use crate::dag::GoalDag;
use crate::errors::{GoalsError, GoalsResult};
use crate::store::GoalStore;
use crate::types::GoalRecord;
use crate::validator::GoalValidator;
use brain_core::ids::GoalId;
use brain_core::types::{GoalPriority, GoalStatus};
use memory_core::Timestamp;
use std::sync::Arc;

pub struct GoalManager {
    store: Arc<dyn GoalStore>,
    dag: std::sync::RwLock<GoalDag>,
    validator: GoalValidator,
}

impl GoalManager {
    pub fn new(store: Arc<dyn GoalStore>) -> Self {
        Self { store, dag: std::sync::RwLock::new(GoalDag::new()), validator: GoalValidator::new() }
    }

    pub async fn create_goal(
        &self,
        goal_id: GoalId,
        goal_type: impl Into<String>,
        description: impl Into<String>,
        priority: GoalPriority,
        dependencies: Vec<GoalId>,
    ) -> GoalsResult<GoalRecord> {
        let existing = self.store.list_all().await?;
        let existing_ids: Vec<GoalId> = existing.iter().map(|g| g.goal_id).collect();

        let mut record = GoalRecord::new(goal_id, goal_type, description, priority);
        record.dependencies = dependencies;

        self.validator.validate_new(&record, &existing_ids)?;

        {
            let mut dag = self.dag.write().map_err(|e| GoalsError::Internal(e.to_string()))?;
            dag.add_node(record.goal_id);

            for dep in &record.dependencies {
                if !dag.has_node(dep) {
                    return Err(GoalsError::DependencyNotFound { goal: record.goal_id, dependency: *dep });
                }
                dag.add_dependency(record.goal_id, *dep)?;
            }
        }

        self.store.insert(record.clone()).await?;
        Ok(record)
    }

    pub async fn activate_goal(&self, goal_id: &GoalId) -> GoalsResult<GoalRecord> {
        let mut record = self.store.get(goal_id).await?;
        let mut dep_statuses = Vec::with_capacity(record.dependencies.len());
        for dep in &record.dependencies {
            if let Ok(r) = self.store.get(dep).await {
                dep_statuses.push(r.status);
            }
        }

        if !self.validator.can_activate(&record, &dep_statuses) {
            return Err(GoalsError::ValidationError(
                format!("goal {:?} is not ready to activate; dependencies not satisfied", goal_id)
            ));
        }

        record.status = GoalStatus::Active;
        record.updated_at = Timestamp::now();
        self.store.update(record.clone()).await?;
        Ok(record)
    }

    pub async fn complete_goal(&self, goal_id: &GoalId, outcome: impl Into<String>) -> GoalsResult<GoalRecord> {
        let mut record = self.store.get(goal_id).await?;
        record.status = GoalStatus::Completed;
        record.completed_at = Some(Timestamp::now());
        record.updated_at = Timestamp::now();
        record.metadata.insert("outcome".into(), outcome.into());
        self.store.update(record.clone()).await?;
        Ok(record)
    }

    pub async fn fail_goal(&self, goal_id: &GoalId, reason: impl Into<String>) -> GoalsResult<GoalRecord> {
        let mut record = self.store.get(goal_id).await?;
        record.status = GoalStatus::Failed;
        record.failure_count += 1;
        record.updated_at = Timestamp::now();
        record.metadata.insert("failure_reason".into(), reason.into());
        self.store.update(record.clone()).await?;
        Ok(record)
    }

    pub async fn cancel_goal(&self, goal_id: &GoalId, reason: impl Into<String>) -> GoalsResult<GoalRecord> {
        let mut record = self.store.get(goal_id).await?;
        if record.status == GoalStatus::Completed {
            return Err(GoalsError::ValidationError("cannot cancel a completed goal".into()));
        }
        record.status = GoalStatus::Cancelled;
        record.updated_at = Timestamp::now();
        record.metadata.insert("cancel_reason".into(), reason.into());
        self.store.update(record.clone()).await?;
        Ok(record)
    }

    pub async fn pause_goal(&self, goal_id: &GoalId, reason: Option<impl Into<String>>) -> GoalsResult<GoalRecord> {
        let mut record = self.store.get(goal_id).await?;
        record.status = GoalStatus::Paused;
        record.updated_at = Timestamp::now();
        if let Some(r) = reason {
            record.metadata.insert("pause_reason".into(), r.into());
        }
        self.store.update(record.clone()).await?;
        Ok(record)
    }

    pub async fn recover_goal(&self, goal_id: &GoalId) -> GoalsResult<GoalRecord> {
        let mut record = self.store.get(goal_id).await?;
        if record.status != GoalStatus::Failed && record.status != GoalStatus::Recovering {
            return Err(GoalsError::ValidationError(
                format!("goal {:?} cannot be recovered from status {:?}", goal_id, record.status)
            ));
        }
        record.status = GoalStatus::Recovering;
        record.recovery_attempts += 1;
        record.updated_at = Timestamp::now();
        self.store.update(record.clone()).await?;
        Ok(record)
    }

    pub async fn get_goal(&self, goal_id: &GoalId) -> GoalsResult<GoalRecord> {
        self.store.get(goal_id).await
    }

    pub async fn list_goals(&self) -> GoalsResult<Vec<GoalRecord>> {
        self.store.list_all().await
    }

    pub async fn list_ready_goals(&self) -> GoalsResult<Vec<GoalRecord>> {
        self.store.list_ready().await
    }

    pub async fn add_dependency(&self, goal: GoalId, dependency: GoalId) -> GoalsResult<()> {
        {
            let mut dag = self.dag.write().map_err(|e| GoalsError::Internal(e.to_string()))?;
            dag.add_node(goal);
            dag.add_node(dependency);
            dag.add_dependency(goal, dependency)?;
        }

        let mut record = self.store.get(&goal).await?;
        if !record.dependencies.contains(&dependency) {
            record.dependencies.push(dependency);
            record.updated_at = Timestamp::now();
            self.store.update(record).await?;
        }
        Ok(())
    }

    pub fn dag(&self) -> Result<GoalDag, GoalsError> {
        self.dag.read().map(|d| d.clone()).map_err(|e| GoalsError::Internal(e.to_string()))
    }
}
