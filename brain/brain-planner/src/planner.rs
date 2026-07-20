use crate::errors::{PlannerError, PlannerResult};
use crate::optimizer::PlanOptimizer;
use crate::selector::ToolSelector;
use crate::task_graph::TaskGraphBuilder;
use crate::types::{AlternativePlan, PlanValidation, TaskGraph, ToolMatch};
use brain_core::budget::CognitiveBudget;
use brain_core::ids::{GoalId, PlanId};
use brain_core::tool::{ExecutablePlan, ToolRegistry, ToolRequirement};
use brain_core::BrainResult;
use std::sync::Arc;

pub struct Planner {
    graph_builder: TaskGraphBuilder,
    optimizer: PlanOptimizer,
    selector: ToolSelector,
    tool_registry: Arc<dyn ToolRegistry>,
}

impl Planner {
    pub fn new(tool_registry: Arc<dyn ToolRegistry>) -> Self {
        Self {
            graph_builder: TaskGraphBuilder::new(),
            optimizer: PlanOptimizer::new(),
            selector: ToolSelector::new(),
            tool_registry,
        }
    }

    pub async fn create_plan(
        &self,
        goal_id: GoalId,
        requirements: Vec<ToolRequirement>,
        _budget: &CognitiveBudget,
    ) -> PlannerResult<ExecutablePlan> {
        let graph = self.graph_builder.build_linear(goal_id, requirements);
        self.graph_builder.validate(&graph)?;

        let optimized = self.optimizer.optimize_duration(&graph);
        let validated = self.validate_plan(&optimized);

        if !validated.is_valid {
            return Err(PlannerError::ValidationError(
                validated.errors.join("; ")
            ));
        }

        let plan_id = PlanId::new();
        let reqs: Vec<ToolRequirement> = optimized.nodes.iter()
            .map(|n| n.tool_requirement.clone())
            .collect();

        let mut plan = ExecutablePlan::new(plan_id, goal_id, reqs);
        plan.estimated_cost = optimized.total_cost();
        plan.estimated_duration_ms = optimized.total_duration_ms();
        Ok(plan)
    }

    pub async fn generate_alternatives(
        &self,
        goal_id: GoalId,
        requirements: Vec<ToolRequirement>,
        _budget: &CognitiveBudget,
        count: usize,
    ) -> PlannerResult<Vec<AlternativePlan>> {
        let graph = self.graph_builder.build_linear(goal_id, requirements);
        self.graph_builder.validate(&graph)?;
        Ok(self.optimizer.generate_alternatives(&graph, count))
    }

    pub async fn select_tools(&self, plan: &ExecutablePlan) -> BrainResult<Vec<ToolMatch>> {
        let mut matches = Vec::new();
        for req in &plan.tool_requirements {
            let candidates = self.selector.select_for_requirement(req, &*self.tool_registry).await?;
            if let Some(best) = self.selector.best_match(&candidates) {
                matches.push(ToolMatch {
                    requirement_id: req.requirement_id.to_string(),
                    capability: req.capability,
                    tool_id: best.tool_id.to_string(),
                    confidence: self.selector.score_match(best, req),
                });
            }
        }
        Ok(matches)
    }

    pub fn validate_plan(&self, graph: &TaskGraph) -> PlanValidation {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        if graph.nodes.is_empty() {
            errors.push("plan has no tasks".into());
        }

        for node in &graph.nodes {
            if node.estimated_cost < 0.0 {
                errors.push(format!("node {} has negative cost", node.id));
            }
            for dep in &node.dependencies {
                if !graph.nodes.iter().any(|n| &n.id == dep) {
                    errors.push(format!("node {} depends on missing node {}", node.id, dep));
                }
            }
        }

        if errors.is_empty() && graph.nodes.len() > 1 {
            let critical = graph.critical_path_duration();
            let total = graph.nodes.iter().map(|n| n.estimated_duration_ms).sum::<u64>();
            if critical < total / 2 {
                warnings.push("plan has significant parallelism opportunity".into());
            }
        }

        PlanValidation {
            is_valid: errors.is_empty(),
            errors,
            warnings,
            node_count: graph.nodes.len(),
            edge_count: graph.nodes.iter().map(|n| n.dependencies.len()).sum(),
            critical_path_length: graph.critical_path_duration(),
        }
    }
}
