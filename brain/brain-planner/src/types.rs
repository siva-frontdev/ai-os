use brain_core::ids::{PlanId, ToolCapabilityId};
use brain_core::tool::ToolRequirement;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskNode {
    pub id: String,
    pub description: String,
    pub tool_requirement: ToolRequirement,
    pub estimated_duration_ms: u64,
    pub estimated_cost: f64,
    pub dependencies: Vec<String>,
    pub is_critical: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskGraph {
    pub nodes: Vec<TaskNode>,
    pub entry_points: Vec<String>,
    pub exit_points: Vec<String>,
}

impl TaskGraph {
    pub fn new(nodes: Vec<TaskNode>) -> Self {
        let entry_points: Vec<String> = nodes.iter()
            .filter(|n| n.dependencies.is_empty())
            .map(|n| n.id.clone())
            .collect();
        let exit_points: Vec<String> = nodes.iter()
            .filter(|n| !nodes.iter().any(|other| other.dependencies.contains(&n.id)))
            .map(|n| n.id.clone())
            .collect();
        TaskGraph { nodes, entry_points, exit_points }
    }

    pub fn total_duration_ms(&self) -> u64 {
        self.critical_path_duration()
    }

    pub fn total_cost(&self) -> f64 {
        self.nodes.iter().map(|n| n.estimated_cost).sum()
    }

    pub fn critical_path_duration(&self) -> u64 {
        let mut longest: HashMap<&str, u64> = HashMap::new();
        let order = self.topological_order();
        for id in &order {
            let node = self.nodes.iter().find(|n| &n.id == id).unwrap();
            let node_dur = node.estimated_duration_ms;
            let pred_max = if node.dependencies.is_empty() {
                0
            } else {
                node.dependencies.iter()
                    .filter_map(|d| longest.get(d.as_str()))
                    .max()
                    .copied()
                    .unwrap_or(0)
            };
            longest.insert(id.as_str(), pred_max + node_dur);
        }
        longest.values().max().copied().unwrap_or(0)
    }

    #[allow(clippy::collapsible_if)]
    pub fn topological_order(&self) -> Vec<String> {
        let mut in_degree: std::collections::HashMap<&str, usize> = self.nodes.iter()
            .map(|n| (n.id.as_str(), n.dependencies.len()))
            .collect();
        let mut queue: Vec<&str> = in_degree.iter()
            .filter(|&(_, deg)| *deg == 0)
            .map(|(id, _)| *id)
            .collect();
        let mut sorted = Vec::new();
        while let Some(id) = queue.pop() {
            sorted.push(id.to_string());
            for other in &self.nodes {
                // clippy prefers collapsing but the && let pattern is unclear here
                if other.dependencies.contains(&id.to_string()) {
                    if let Some(deg) = in_degree.get_mut(other.id.as_str()) {
                        *deg = deg.saturating_sub(1);
                        if *deg == 0 {
                            queue.push(other.id.as_str());
                        }
                    }
                }
            }
        }
        sorted
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlternativePlan {
    pub plan_id: PlanId,
    pub task_graph: TaskGraph,
    pub total_cost: f64,
    pub total_duration_ms: u64,
    pub risk_score: f64,
    pub tool_matches: Vec<ToolMatch>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolMatch {
    pub requirement_id: String,
    pub capability: ToolCapabilityId,
    pub tool_id: String,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanValidation {
    pub is_valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub node_count: usize,
    pub edge_count: usize,
    pub critical_path_length: u64,
}
