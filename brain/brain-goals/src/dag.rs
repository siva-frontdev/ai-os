use crate::errors::{GoalsError, GoalsResult};
use brain_core::ids::GoalId;
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Debug, Clone)]
pub struct GoalDag {
    edges: HashMap<GoalId, HashSet<GoalId>>,
    reverse: HashMap<GoalId, HashSet<GoalId>>,
}

impl GoalDag {
    pub fn new() -> Self {
        Self {
            edges: HashMap::new(),
            reverse: HashMap::new(),
        }
    }

    pub fn add_node(&mut self, id: GoalId) {
        self.edges.entry(id).or_default();
        self.reverse.entry(id).or_default();
    }

    pub fn remove_node(&mut self, id: &GoalId) {
        if let Some(deps) = self.edges.remove(id) {
            for dep in deps {
                if let Some(rev) = self.reverse.get_mut(&dep) {
                    rev.remove(id);
                }
            }
        }
        if let Some(rev) = self.reverse.remove(id) {
            for dep in rev {
                if let Some(edges) = self.edges.get_mut(&dep) {
                    edges.remove(id);
                }
            }
        }
    }

    pub fn add_dependency(&mut self, from: GoalId, to: GoalId) -> GoalsResult<()> {
        if from == to {
            return Err(GoalsError::ValidationError(
                "goal cannot depend on itself".into(),
            ));
        }
        if self.would_create_cycle(from, to) {
            let cycle = self.detect_cycle_path(from, to);
            return Err(GoalsError::CycleDetected(cycle));
        }
        self.edges.entry(from).or_default().insert(to);
        self.reverse.entry(to).or_default().insert(from);
        Ok(())
    }

    pub fn remove_dependency(&mut self, from: &GoalId, to: &GoalId) {
        if let Some(deps) = self.edges.get_mut(from) {
            deps.remove(to);
        }
        if let Some(rev) = self.reverse.get_mut(to) {
            rev.remove(from);
        }
    }

    pub fn dependencies(&self, id: &GoalId) -> Vec<GoalId> {
        self.edges
            .get(id)
            .map(|s| s.iter().copied().collect())
            .unwrap_or_default()
    }

    pub fn dependents(&self, id: &GoalId) -> Vec<GoalId> {
        self.reverse
            .get(id)
            .map(|s| s.iter().copied().collect())
            .unwrap_or_default()
    }

    pub fn has_node(&self, id: &GoalId) -> bool {
        self.edges.contains_key(id)
    }

    pub fn topological_sort(&self) -> GoalsResult<Vec<GoalId>> {
        let mut in_degree: HashMap<GoalId, usize> = HashMap::new();
        for id in self.edges.keys() {
            in_degree.entry(*id).or_insert(0);
        }
        for deps in self.edges.values() {
            for dep in deps {
                *in_degree.entry(*dep).or_insert(0) += 1;
            }
        }

        let mut queue: VecDeque<GoalId> = in_degree
            .iter()
            .filter(|&(_, deg)| *deg == 0)
            .map(|(id, _)| *id)
            .collect();

        let mut sorted = Vec::with_capacity(in_degree.len());
        while let Some(id) = queue.pop_front() {
            sorted.push(id);
            if let Some(deps) = self.edges.get(&id) {
                for dep in deps {
                    if let Some(deg) = in_degree.get_mut(dep) {
                        *deg -= 1;
                        if *deg == 0 {
                            queue.push_back(*dep);
                        }
                    }
                }
            }
        }

        if sorted.len() != in_degree.len() {
            let remaining: Vec<GoalId> = in_degree
                .iter()
                .filter(|&(_, deg)| *deg > 0)
                .map(|(id, _)| *id)
                .collect();
            return Err(GoalsError::CycleDetected(remaining));
        }

        Ok(sorted)
    }

    fn would_create_cycle(&self, from: GoalId, to: GoalId) -> bool {
        let mut visited = HashSet::new();
        let mut stack = vec![to];
        while let Some(current) = stack.pop() {
            if current == from {
                return true;
            }
            if visited.insert(current)
                && let Some(deps) = self.edges.get(&current)
            {
                stack.extend(deps.iter());
            }
        }
        false
    }

    fn detect_cycle_path(&self, from: GoalId, to: GoalId) -> Vec<GoalId> {
        let path = vec![from, to];
        let mut visited = HashSet::new();
        let mut stack = vec![(to, vec![from, to])];
        while let Some((current, current_path)) = stack.pop() {
            if current == from {
                return current_path;
            }
            if visited.insert(current)
                && let Some(deps) = self.edges.get(&current)
            {
                for dep in deps {
                    let mut new_path = current_path.clone();
                    new_path.push(*dep);
                    stack.push((*dep, new_path));
                }
            }
        }
        path
    }

    pub fn is_reachable(&self, from: &GoalId, to: &GoalId) -> bool {
        let mut visited = HashSet::new();
        let mut stack = vec![*from];
        while let Some(current) = stack.pop() {
            if current == *to {
                return true;
            }
            if visited.insert(current)
                && let Some(deps) = self.edges.get(&current)
            {
                stack.extend(deps.iter());
            }
        }
        false
    }

    pub fn node_count(&self) -> usize {
        self.edges.len()
    }

    pub fn edge_count(&self) -> usize {
        self.edges.values().map(|s| s.len()).sum()
    }
}

impl Default for GoalDag {
    fn default() -> Self {
        Self::new()
    }
}
