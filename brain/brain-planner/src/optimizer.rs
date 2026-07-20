use crate::types::{AlternativePlan, TaskGraph};
use brain_core::ids::PlanId;

#[derive(Debug)]
pub struct PlanOptimizer;

impl PlanOptimizer {
    pub fn new() -> Self { Self }

    pub fn optimize_duration(&self, graph: &TaskGraph) -> TaskGraph {
        let _critical = graph.critical_path_duration();
        let mut nodes = graph.nodes.clone();
        for node in &mut nodes {
            if !node.is_critical {
                node.estimated_duration_ms = node.estimated_duration_ms.saturating_div(2).max(100);
            }
        }
        TaskGraph::new(nodes)
    }

    pub fn optimize_cost(&self, graph: &TaskGraph) -> TaskGraph {
        let mut nodes = graph.nodes.clone();
        for node in &mut nodes {
            if !node.is_critical {
                node.estimated_cost *= 0.5;
            }
        }
        TaskGraph::new(nodes)
    }

    pub fn generate_alternatives(&self, base: &TaskGraph, count: usize) -> Vec<AlternativePlan> {
        let mut alternatives = Vec::with_capacity(count);
        for i in 0..count {
            let variance = 1.0 + (i as f64) * 0.1;
            let mut nodes = base.nodes.clone();
            for node in &mut nodes {
                node.estimated_duration_ms = (node.estimated_duration_ms as f64 * variance) as u64;
                node.estimated_cost *= variance;
            }
            let graph = TaskGraph::new(nodes);
            alternatives.push(AlternativePlan {
                plan_id: PlanId::new(),
                task_graph: graph.clone(),
                total_cost: graph.total_cost(),
                total_duration_ms: graph.total_duration_ms(),
                risk_score: 0.1 * (i as f64),
                tool_matches: Vec::new(),
            });
        }
        alternatives
    }
}

impl Default for PlanOptimizer {
    fn default() -> Self { Self::new() }
}
