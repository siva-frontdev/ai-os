use crate::dag::GoalDag;
use crate::errors::{GoalsError, GoalsResult};
use crate::types::GoalRecord;
use brain_core::ids::GoalId;
use brain_core::types::GoalStatus;

#[derive(Debug, Clone)]
pub struct GoalValidator;

impl GoalValidator {
    pub fn new() -> Self { Self }

    pub fn validate_new(&self, record: &GoalRecord, existing: &[GoalId]) -> GoalsResult<()> {
        if record.description.is_empty() {
            return Err(GoalsError::ValidationError("goal description must not be empty".into()));
        }
        if existing.contains(&record.goal_id) {
            return Err(GoalsError::AlreadyExists(record.goal_id));
        }
        Ok(())
    }

    pub fn validate_dag(&self, dag: &GoalDag) -> GoalsResult<()> {
        dag.topological_sort()?;
        Ok(())
    }

    pub fn validate_dependencies_exist(&self, dependencies: &[GoalId], dag: &GoalDag) -> GoalsResult<()> {
        for dep in dependencies {
            if !dag.has_node(dep) {
                return Err(GoalsError::DependencyNotFound {
                    goal: dependencies[0],
                    dependency: *dep,
                });
            }
        }
        Ok(())
    }

    pub fn can_activate(&self, record: &GoalRecord, dep_statuses: &[GoalStatus]) -> bool {
        if record.status != GoalStatus::Pending {
            return false;
        }
        if !record.dependencies.is_empty() && dep_statuses.len() != record.dependencies.len() {
            return false;
        }
        dep_statuses.iter().all(|s| matches!(s, GoalStatus::Completed))
    }
}

impl Default for GoalValidator {
    fn default() -> Self { Self::new() }
}
