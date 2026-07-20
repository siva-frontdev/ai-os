use crate::errors::{PlannerError, PlannerResult};
use crate::types::{TaskGraph, TaskNode};
use brain_core::ids::GoalId;
use brain_core::tool::ToolRequirement;

#[derive(Debug)]
pub struct TaskGraphBuilder;

impl TaskGraphBuilder {
    pub fn new() -> Self {
        Self
    }

    pub fn build_linear(&self, goal_id: GoalId, requirements: Vec<ToolRequirement>) -> TaskGraph {
        let mut nodes = Vec::with_capacity(requirements.len());
        let mut prev_id: Option<String> = None;
        for (i, req) in requirements.into_iter().enumerate() {
            let id = format!("{}-step-{}", goal_id, i);
            let mut deps = Vec::new();
            if let Some(ref prev) = prev_id {
                deps.push(prev.clone());
            }
            nodes.push(TaskNode {
                id: id.clone(),
                description: format!("step {}", i),
                tool_requirement: req.clone(),
                estimated_duration_ms: 1000,
                estimated_cost: 0.0,
                dependencies: deps,
                is_critical: true,
            });
            prev_id = Some(id);
        }
        TaskGraph::new(nodes)
    }

    pub fn build_parallel(
        &self,
        goal_id: GoalId,
        requirements: Vec<Vec<ToolRequirement>>,
    ) -> TaskGraph {
        let mut nodes = Vec::new();
        for (group_idx, group) in requirements.iter().enumerate() {
            let group_deps: Vec<String> = (0..group_idx)
                .flat_map(|i| {
                    (0..requirements[i].len()).map(move |j| format!("{}-step-{}-{}", goal_id, i, j))
                })
                .collect();
            for (step_idx, req) in group.iter().enumerate() {
                nodes.push(TaskNode {
                    id: format!("{}-step-{}-{}", goal_id, group_idx, step_idx),
                    description: format!("group {} step {}", group_idx, step_idx),
                    tool_requirement: req.clone(),
                    estimated_duration_ms: 1000,
                    estimated_cost: 0.0,
                    dependencies: group_deps.clone(),
                    is_critical: false,
                });
            }
        }
        TaskGraph::new(nodes)
    }

    pub fn validate(&self, graph: &TaskGraph) -> PlannerResult<()> {
        for node in &graph.nodes {
            if node.id.is_empty() {
                return Err(PlannerError::ValidationError(
                    "node id must not be empty".into(),
                ));
            }
            for dep in &node.dependencies {
                if !graph.nodes.iter().any(|n| &n.id == dep) {
                    return Err(PlannerError::ValidationError(format!(
                        "dependency {} not found for node {}",
                        dep, node.id
                    )));
                }
            }
        }
        if graph.nodes.is_empty() {
            return Err(PlannerError::ValidationError(
                "task graph must have at least one node".into(),
            ));
        }
        Ok(())
    }
}

impl Default for TaskGraphBuilder {
    fn default() -> Self {
        Self::new()
    }
}
