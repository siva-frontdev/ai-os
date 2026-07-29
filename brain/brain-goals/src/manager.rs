use crate::dag::GoalDag;
use crate::errors::{GoalsError, GoalsResult};
use crate::store::GoalStore;
use crate::types::{GoalOutcome, GoalRecord, MilestoneRecord, ObjectiveRecord};
use crate::validator::GoalValidator;
use brain_core::ids::{GoalId, MilestoneId, ObjectiveId, StrategyId};
use brain_core::strategy::ExecutionStrategy;
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
        Self {
            store,
            dag: std::sync::RwLock::new(GoalDag::new()),
            validator: GoalValidator::new(),
        }
    }

    /// Access the underlying goal store (for recovery/persistence operations).
    pub fn store(&self) -> &Arc<dyn GoalStore> {
        &self.store
    }

    // ── Core lifecycle ───────────────────────────────────────

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
            let mut dag = self
                .dag
                .write()
                .map_err(|e| GoalsError::Internal(e.to_string()))?;
            dag.add_node(record.goal_id);

            for dep in &record.dependencies {
                if !dag.has_node(dep) {
                    return Err(GoalsError::DependencyNotFound {
                        goal: record.goal_id,
                        dependency: *dep,
                    });
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
            return Err(GoalsError::ValidationError(format!(
                "goal {:?} is not ready to activate; dependencies not satisfied",
                goal_id
            )));
        }

        record.status = GoalStatus::Active;
        record.started_at = record.started_at.or(Some(Timestamp::now()));
        record.updated_at = Timestamp::now();
        record.version += 1;
        self.store.update(record.clone()).await?;
        Ok(record)
    }

    pub async fn complete_goal(
        &self,
        goal_id: &GoalId,
        outcome: GoalOutcome,
    ) -> GoalsResult<GoalRecord> {
        let mut record = self.store.get(goal_id).await?;
        record.status = GoalStatus::Completed;
        record.completed_at = Some(Timestamp::now());
        record.updated_at = Timestamp::now();
        record.progress_pct = 1.0;
        record.outcome = Some(outcome);
        record.version += 1;
        self.store.update(record.clone()).await?;
        Ok(record)
    }

    pub async fn fail_goal(
        &self,
        goal_id: &GoalId,
        reason: impl Into<String>,
    ) -> GoalsResult<GoalRecord> {
        let mut record = self.store.get(goal_id).await?;
        record.status = GoalStatus::Failed;
        record.failure_count += 1;
        record.updated_at = Timestamp::now();
        record.last_error = Some(reason.into());
        record.version += 1;
        self.store.update(record.clone()).await?;
        Ok(record)
    }

    pub async fn cancel_goal(
        &self,
        goal_id: &GoalId,
        reason: impl Into<String>,
    ) -> GoalsResult<GoalRecord> {
        let mut record = self.store.get(goal_id).await?;
        if record.status == GoalStatus::Completed {
            return Err(GoalsError::ValidationError(
                "cannot cancel a completed goal".into(),
            ));
        }
        record.status = GoalStatus::Cancelled;
        record.updated_at = Timestamp::now();
        record.version += 1;
        record
            .metadata
            .insert("cancel_reason".into(), reason.into());
        self.store.update(record.clone()).await?;
        Ok(record)
    }

    pub async fn pause_goal(
        &self,
        goal_id: &GoalId,
        reason: Option<impl Into<String>>,
    ) -> GoalsResult<GoalRecord> {
        let mut record = self.store.get(goal_id).await?;
        if !matches!(
            record.status,
            GoalStatus::Active | GoalStatus::Executing | GoalStatus::Planning
        ) {
            return Err(GoalsError::ValidationError(format!(
                "cannot pause goal {:?} in state {:?}",
                goal_id, record.status
            )));
        }
        record.status = GoalStatus::Paused;
        record.updated_at = Timestamp::now();
        record.version += 1;
        if let Some(r) = reason {
            record.metadata.insert("pause_reason".into(), r.into());
        }
        self.store.update(record.clone()).await?;
        Ok(record)
    }

    pub async fn recover_goal(&self, goal_id: &GoalId) -> GoalsResult<GoalRecord> {
        let mut record = self.store.get(goal_id).await?;
        if record.status != GoalStatus::Failed && record.status != GoalStatus::Recovering {
            return Err(GoalsError::ValidationError(format!(
                "goal {:?} cannot be recovered from status {:?}",
                goal_id, record.status
            )));
        }
        record.status = GoalStatus::Recovering;
        record.recovery_attempts += 1;
        record.updated_at = Timestamp::now();
        record.version += 1;
        self.store.update(record.clone()).await?;
        Ok(record)
    }

    // ── Extended lifecycle ───────────────────────────────────

    pub async fn merge_goals(
        &self,
        target_id: GoalId,
        source_id: GoalId,
    ) -> GoalsResult<GoalRecord> {
        if target_id == source_id {
            return Err(GoalsError::ValidationError(
                "cannot merge a goal with itself".into(),
            ));
        }
        let mut target = self.store.get(&target_id).await?;
        let source = self.store.get(&source_id).await?;

        if target.status == GoalStatus::Completed || source.status == GoalStatus::Completed {
            return Err(GoalsError::ValidationError(
                "cannot merge completed goals".into(),
            ));
        }

        target.children.extend(source.children);
        let new_deps: Vec<GoalId> = source
            .dependencies
            .iter()
            .filter(|d| !target.dependencies.contains(d))
            .copied()
            .collect();
        target.dependencies.extend(new_deps);
        target
            .metadata
            .insert("merged_from".into(), source_id.to_string());
        target
            .metadata
            .insert("merged_source_description".into(), source.description);
        target.updated_at = Timestamp::now();
        target.version += 1;

        self.store.update(target.clone()).await?;
        self.store.delete(&source_id).await?;

        {
            let mut dag = self
                .dag
                .write()
                .map_err(|e| GoalsError::Internal(e.to_string()))?;
            dag.remove_node(&source_id);
        }

        Ok(target)
    }

    pub async fn split_goal(
        &self,
        source_id: GoalId,
        new_goal_types: Vec<(&str, &str, GoalPriority)>,
    ) -> GoalsResult<(GoalRecord, Vec<GoalRecord>)> {
        let source = self.store.get(&source_id).await?;
        if new_goal_types.is_empty() {
            return Err(GoalsError::ValidationError(
                "split requires at least one new goal".into(),
            ));
        }

        let mut new_goals: Vec<GoalRecord> = Vec::new();
        let mut first_new_id = None;

        for (i, (gtype, desc, priority)) in new_goal_types.iter().enumerate() {
            let new_id = GoalId::new();
            let mut record = GoalRecord::new(new_id, *gtype, *desc, *priority);
            record.parent = Some(source_id);
            record.source = source.source.clone();
            record.tags = source.tags.clone();
            record.budget = source.budget.clone();
            if i > 0 {
                record.dependencies.push(new_goals[i - 1].goal_id);
            }
            if let Some(first) = first_new_id {
                record.dependencies.push(first);
            } else {
                first_new_id = Some(new_id);
            }
            record.updated_at = Timestamp::now();

            {
                let mut dag = self
                    .dag
                    .write()
                    .map_err(|e| GoalsError::Internal(e.to_string()))?;
                dag.add_node(new_id);
                for dep in &record.dependencies {
                    if dag.has_node(dep) {
                        dag.add_dependency(new_id, *dep)?;
                    }
                }
            }

            self.store.insert(record.clone()).await?;
            new_goals.push(record);
        }

        let mut source = self.store.get(&source_id).await?;
        source.status = GoalStatus::Splitting;
        for ng in &new_goals {
            source.children.push(ng.goal_id);
        }
        source.updated_at = Timestamp::now();
        source.version += 1;
        self.store.update(source.clone()).await?;

        Ok((source, new_goals))
    }

    pub async fn reprioritize_goal(
        &self,
        goal_id: &GoalId,
        new_priority: GoalPriority,
    ) -> GoalsResult<GoalRecord> {
        let mut record = self.store.get(goal_id).await?;
        let old = record.priority;
        if old == new_priority {
            return Ok(record);
        }
        record.priority = new_priority;
        record.updated_at = Timestamp::now();
        record.version += 1;
        record
            .metadata
            .insert("old_priority".into(), format!("{:?}", old));
        self.store.update(record.clone()).await?;
        Ok(record)
    }

    pub async fn archive_goal(
        &self,
        goal_id: &GoalId,
        reason: impl Into<String>,
    ) -> GoalsResult<GoalRecord> {
        let mut record = self.store.get(goal_id).await?;
        if record.status == GoalStatus::Archived {
            return Err(GoalsError::ValidationError(
                "goal is already archived".into(),
            ));
        }
        record.status = GoalStatus::Archived;
        record.updated_at = Timestamp::now();
        record.version += 1;
        record
            .metadata
            .insert("archive_reason".into(), reason.into());
        self.store.update(record.clone()).await?;
        Ok(record)
    }

    pub async fn block_goal(
        &self,
        goal_id: &GoalId,
        blocker: impl Into<String>,
    ) -> GoalsResult<GoalRecord> {
        let mut record = self.store.get(goal_id).await?;
        if record.status == GoalStatus::Blocked {
            return Err(GoalsError::ValidationError(
                "goal is already blocked".into(),
            ));
        }
        record.status = GoalStatus::Blocked;
        record.blocked_reason = Some(blocker.into());
        record.updated_at = Timestamp::now();
        record.version += 1;
        self.store.update(record.clone()).await?;
        Ok(record)
    }

    pub async fn unblock_goal(&self, goal_id: &GoalId) -> GoalsResult<GoalRecord> {
        let mut record = self.store.get(goal_id).await?;
        if record.status != GoalStatus::Blocked {
            return Err(GoalsError::ValidationError(format!(
                "goal {:?} is not blocked",
                goal_id
            )));
        }
        record.status = GoalStatus::Active;
        record.blocked_reason = None;
        record.updated_at = Timestamp::now();
        record.version += 1;
        self.store.update(record.clone()).await?;
        Ok(record)
    }

    pub async fn defer_goal(
        &self,
        goal_id: &GoalId,
        until: Option<Timestamp>,
        reason: impl Into<String>,
    ) -> GoalsResult<GoalRecord> {
        let mut record = self.store.get(goal_id).await?;
        record.status = GoalStatus::Deferred;
        record.deferred_until = until;
        record.updated_at = Timestamp::now();
        record.version += 1;
        record.metadata.insert("defer_reason".into(), reason.into());
        self.store.update(record.clone()).await?;
        Ok(record)
    }

    pub async fn retry_goal(
        &self,
        goal_id: &GoalId,
        reason: impl Into<String>,
    ) -> GoalsResult<GoalRecord> {
        let mut record = self.store.get(goal_id).await?;
        if record.status != GoalStatus::Failed && record.status != GoalStatus::Recovering {
            return Err(GoalsError::ValidationError(format!(
                "only failed goals can be retried, got {:?}",
                record.status
            )));
        }
        record.retry_count += 1;
        record.status = GoalStatus::Active;
        record.started_at = Some(Timestamp::now());
        record.updated_at = Timestamp::now();
        record.version += 1;
        record.metadata.insert("retry_reason".into(), reason.into());
        self.store.update(record.clone()).await?;
        Ok(record)
    }

    // ── Strategy ─────────────────────────────────────────────

    pub async fn assign_strategy(
        &self,
        goal_id: &GoalId,
        strategy_id: StrategyId,
        approach: ExecutionStrategy,
    ) -> GoalsResult<GoalRecord> {
        let mut record = self.store.get(goal_id).await?;
        record.strategy_id = Some(strategy_id);
        record.execution_approach = Some(approach);
        record.updated_at = Timestamp::now();
        record.version += 1;
        self.store.update(record.clone()).await?;
        Ok(record)
    }

    // ── Hierarchical decomposition ───────────────────────────

    pub async fn add_objective(
        &self,
        goal_id: &GoalId,
        description: impl Into<String>,
        order: u32,
    ) -> GoalsResult<(GoalRecord, ObjectiveRecord)> {
        let mut record = self.store.get(goal_id).await?;
        let obj = ObjectiveRecord {
            objective_id: ObjectiveId::new(),
            goal_id: *goal_id,
            description: description.into(),
            order,
            status: GoalStatus::Pending,
            milestones: Vec::new(),
            created_at: Timestamp::now(),
            completed_at: None,
        };
        record.objectives.push(obj.clone());
        record.objectives.sort_by_key(|o| o.order);
        if record.current_objective.is_none() {
            record.current_objective = Some(obj.objective_id);
        }
        record.updated_at = Timestamp::now();
        record.version += 1;
        self.store.update(record.clone()).await?;
        Ok((record, obj))
    }

    pub async fn add_milestone(
        &self,
        goal_id: &GoalId,
        objective_id: &ObjectiveId,
        description: impl Into<String>,
        order: u32,
    ) -> GoalsResult<(GoalRecord, MilestoneRecord)> {
        let mut record = self.store.get(goal_id).await?;
        let obj = record
            .objectives
            .iter_mut()
            .find(|o| o.objective_id == *objective_id)
            .ok_or_else(|| GoalsError::ValidationError("objective not found".into()))?;

        let milestone = MilestoneRecord {
            milestone_id: MilestoneId::new(),
            objective_id: *objective_id,
            description: description.into(),
            order,
            status: GoalStatus::Pending,
            created_at: Timestamp::now(),
            reached_at: None,
        };
        obj.milestones.push(milestone.clone());
        obj.milestones.sort_by_key(|m| m.order);
        record.updated_at = Timestamp::now();
        record.version += 1;
        self.store.update(record.clone()).await?;
        Ok((record, milestone))
    }

    pub async fn complete_objective(
        &self,
        goal_id: &GoalId,
        objective_id: &ObjectiveId,
    ) -> GoalsResult<GoalRecord> {
        let mut record = self.store.get(goal_id).await?;
        let obj = record
            .objectives
            .iter_mut()
            .find(|o| o.objective_id == *objective_id)
            .ok_or_else(|| GoalsError::ValidationError("objective not found".into()))?;
        obj.status = GoalStatus::Completed;
        obj.completed_at = Some(Timestamp::now());
        record.updated_at = Timestamp::now();
        record.version += 1;
        self.store.update(record.clone()).await?;
        Ok(record)
    }

    pub async fn reach_milestone(
        &self,
        goal_id: &GoalId,
        milestone_id: &MilestoneId,
    ) -> GoalsResult<GoalRecord> {
        let mut record = self.store.get(goal_id).await?;
        for obj in &mut record.objectives {
            if let Some(m) = obj
                .milestones
                .iter_mut()
                .find(|m| m.milestone_id == *milestone_id)
            {
                m.status = GoalStatus::Completed;
                m.reached_at = Some(Timestamp::now());
                record.updated_at = Timestamp::now();
                record.version += 1;
                self.store.update(record.clone()).await?;
                return Ok(record);
            }
        }
        Err(GoalsError::ValidationError("milestone not found".into()))
    }

    pub async fn advance_to_next_objective(&self, goal_id: &GoalId) -> GoalsResult<GoalRecord> {
        let mut record = self.store.get(goal_id).await?;
        let current_idx = record.current_objective.and_then(|current| {
            record
                .objectives
                .iter()
                .position(|o| o.objective_id == current)
        });
        let next_idx = current_idx.map(|i| i + 1).unwrap_or(0);
        if next_idx < record.objectives.len() {
            record.current_objective = Some(record.objectives[next_idx].objective_id);
        } else {
            record.current_objective = None;
        }
        record.updated_at = Timestamp::now();
        record.version += 1;
        self.store.update(record.clone()).await?;
        Ok(record)
    }

    pub async fn update_progress(
        &self,
        goal_id: &GoalId,
        progress_pct: f64,
    ) -> GoalsResult<GoalRecord> {
        let mut record = self.store.get(goal_id).await?;
        record.progress_pct = progress_pct.clamp(0.0, 1.0);
        record.updated_at = Timestamp::now();
        self.store.update(record.clone()).await?;
        Ok(record)
    }

    // ── Query ────────────────────────────────────────────────

    pub async fn get_goal(&self, goal_id: &GoalId) -> GoalsResult<GoalRecord> {
        self.store.get(goal_id).await
    }

    pub async fn list_goals(&self) -> GoalsResult<Vec<GoalRecord>> {
        self.store.list_all().await
    }

    pub async fn list_active_goals(&self) -> GoalsResult<Vec<GoalRecord>> {
        self.store.list_active().await
    }

    pub async fn list_ready_goals(&self) -> GoalsResult<Vec<GoalRecord>> {
        self.store.list_ready().await
    }

    pub async fn search_by_tag(&self, tag: &str) -> GoalsResult<Vec<GoalRecord>> {
        self.store.search_by_tag(tag).await
    }

    pub async fn list_since(&self, since: Timestamp) -> GoalsResult<Vec<GoalRecord>> {
        self.store.list_since(since).await
    }

    pub async fn add_dependency(&self, goal: GoalId, dependency: GoalId) -> GoalsResult<()> {
        {
            let mut dag = self
                .dag
                .write()
                .map_err(|e| GoalsError::Internal(e.to_string()))?;
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

    pub async fn add_tag(
        &self,
        goal_id: &GoalId,
        tag: impl Into<String>,
    ) -> GoalsResult<GoalRecord> {
        let mut record = self.store.get(goal_id).await?;
        let tag_str = tag.into();
        if !record.tags.contains(&tag_str) {
            record.tags.push(tag_str);
            record.updated_at = Timestamp::now();
            self.store.update(record.clone()).await?;
        }
        Ok(record)
    }

    pub async fn snapshot(&self, label: &str) -> GoalsResult<()> {
        self.store.save_snapshot(label).await
    }

    pub async fn restore(&self, label: &str) -> GoalsResult<()> {
        self.store.restore_snapshot(label).await
    }

    pub fn dag(&self) -> Result<GoalDag, GoalsError> {
        self.dag
            .read()
            .map(|d| d.clone())
            .map_err(|e| GoalsError::Internal(e.to_string()))
    }
}
