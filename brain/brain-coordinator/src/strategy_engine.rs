use brain_core::ids::StrategyId;
use brain_core::model::ResourceState;
use brain_core::strategy::{ExecutionStrategy, StrategyProfile};

use std::collections::HashMap;
use std::sync::RwLock;

/// Selects the optimal execution strategy for a goal based on its
/// type, priority, available resources, and past learning.
#[derive(Debug)]
pub struct StrategyEngine {
    profiles: RwLock<HashMap<StrategyId, StrategyProfile>>,
    goal_type_rules: RwLock<HashMap<String, ExecutionStrategy>>,
}

impl StrategyEngine {
    pub fn new() -> Self {
        let mut engine = Self {
            profiles: RwLock::new(HashMap::new()),
            goal_type_rules: RwLock::new(HashMap::new()),
        };
        engine.register_default_rules();
        engine
    }

    fn register_default_rules(&mut self) {
        let mut rules = self.goal_type_rules.write().expect("strategy rules lock");
        rules.insert("research".into(), ExecutionStrategy::ResearchThenExecute);
        rules.insert("browsing".into(), ExecutionStrategy::Direct);
        rules.insert("file_management".into(), ExecutionStrategy::Direct);
        rules.insert("development".into(), ExecutionStrategy::Sequential);
        rules.insert("system_configuration".into(), ExecutionStrategy::Sequential);
        rules.insert("data_analysis".into(), ExecutionStrategy::Parallel);
        rules.insert("monitoring".into(), ExecutionStrategy::Iterative);
        rules.insert("learning".into(), ExecutionStrategy::Iterative);
        rules.insert("deployment".into(), ExecutionStrategy::Sequential);
        rules.insert("unknown".into(), ExecutionStrategy::ResearchThenExecute);
    }

    // ── Profile management ───────────────────────────────────

    pub fn register_profile(&self, profile: StrategyProfile) {
        let mut profiles = self.profiles.write().expect("strategy profiles lock");
        profiles.insert(profile.strategy_id, profile);
    }

    pub fn get_profile(&self, strategy_id: &StrategyId) -> Option<StrategyProfile> {
        let profiles = self.profiles.read().expect("strategy profiles lock");
        profiles.get(strategy_id).cloned()
    }

    pub fn list_profiles(&self) -> Vec<StrategyProfile> {
        let profiles = self.profiles.read().expect("strategy profiles lock");
        profiles.values().cloned().collect()
    }

    // ── Strategy selection ───────────────────────────────────

    /// Select the best execution strategy for a goal type given
    /// the current resource state and goal priority.
    pub fn select_strategy(
        &self,
        goal_type: &str,
        priority: u8,
        resources: ResourceState,
    ) -> (StrategyProfile, ExecutionStrategy) {
        let approach = self.approach_for_goal_type(goal_type);

        // Degrade strategy when resources are constrained
        let adjusted = match (resources, &approach) {
            (ResourceState::Exhausted, _) => ExecutionStrategy::HumanAssisted,
            (ResourceState::Critical, ExecutionStrategy::Parallel) => ExecutionStrategy::Sequential,
            (ResourceState::Critical, ExecutionStrategy::ResearchThenExecute) => {
                ExecutionStrategy::Direct
            }
            (ResourceState::Degraded, ExecutionStrategy::Parallel) if priority < 5 => {
                ExecutionStrategy::Sequential
            }
            _ => approach,
        };

        let strategy_id = StrategyId::new();
        let profile = StrategyProfile {
            strategy_id,
            name: format!("strategy-{}", goal_type),
            description: format!("Default {} execution strategy", goal_type),
            approach: adjusted,
            risk_tolerance: self.risk_for_priority(priority),
            exploration_bias: self.exploration_for_goal_type(goal_type),
            max_delegation_depth: if matches!(resources, ResourceState::Healthy) {
                3
            } else {
                1
            },
            requires_human_approval: matches!(resources, ResourceState::Exhausted),
        };

        self.register_profile(profile.clone());
        (profile, adjusted)
    }

    fn approach_for_goal_type(&self, goal_type: &str) -> ExecutionStrategy {
        let rules = self.goal_type_rules.read().expect("strategy rules lock");
        let normalized = goal_type.to_lowercase().replace(' ', "_");
        rules
            .get(&normalized)
            .copied()
            .unwrap_or(ExecutionStrategy::ResearchThenExecute)
    }

    fn risk_for_priority(&self, priority: u8) -> f64 {
        match priority {
            0..=3 => 0.8,
            4..=6 => 0.5,
            _ => 0.2,
        }
    }

    fn exploration_for_goal_type(&self, goal_type: &str) -> f64 {
        match goal_type {
            t if t.contains("research") || t.contains("learn") => 0.7,
            t if t.contains("deploy") || t.contains("configure") => 0.2,
            _ => 0.4,
        }
    }

    // ── Goal type rules ──────────────────────────────────────

    pub fn set_rule(&self, goal_type: &str, strategy: ExecutionStrategy) {
        let mut rules = self.goal_type_rules.write().expect("strategy rules lock");
        rules.insert(goal_type.into(), strategy);
    }

    pub fn get_rule(&self, goal_type: &str) -> Option<ExecutionStrategy> {
        let rules = self.goal_type_rules.read().expect("strategy rules lock");
        rules.get(goal_type).copied()
    }
}

impl Default for StrategyEngine {
    fn default() -> Self {
        Self::new()
    }
}
