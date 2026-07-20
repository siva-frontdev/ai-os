#[cfg(test)]
mod tests {
    use brain_core::ids::{GoalId, ToolCapabilityId};
    use brain_core::tool::{ToolCandidate, ToolCapability, ToolRegistry, ToolRequirement};
    use brain_core::types::Confidence;
    use brain_core::BrainResult;
    use std::collections::HashMap;
    use std::sync::Arc;
    use uuid::Uuid;
        use crate::optimizer::PlanOptimizer;
    use crate::planner::Planner;
    use crate::selector::ToolSelector;
    use crate::task_graph::TaskGraphBuilder;
    use crate::types::{TaskGraph, TaskNode};

    struct MockToolRegistry {
        candidates: Vec<ToolCandidate>,
    }

    impl MockToolRegistry {
        fn new() -> Self { Self { candidates: Vec::new() } }
        fn with(mut self, c: ToolCandidate) -> Self { self.candidates.push(c); self }
    }

    #[async_trait::async_trait]
    impl ToolRegistry for MockToolRegistry {
        async fn find_candidates(&self, _capability: ToolCapabilityId) -> BrainResult<Vec<ToolCandidate>> {
            Ok(self.candidates.clone())
        }
        async fn register(&self, _candidate: ToolCandidate) -> BrainResult<()> {
            Ok(())
        }
        async fn deregister(&self, _tool_id: brain_core::ids::ToolId) -> BrainResult<()> {
            Ok(())
        }
        async fn list_capabilities(&self) -> BrainResult<Vec<ToolCapability>> {
            Ok(Vec::new())
        }
    }

    fn make_goal_id(n: u8) -> GoalId {
        GoalId::from(Uuid::from_bytes({
            let mut buf = [0u8; 16];
            buf[0] = n;
            buf[15] = n;
            buf
        }))
    }

    fn make_req(id: &str, capability: ToolCapabilityId) -> ToolRequirement {
        ToolRequirement::new(capability, HashMap::new(), vec![id.into()])
    }

    #[test]
    fn test_task_graph_new() {
        let nodes = vec![
            TaskNode { id: "a".into(), description: "A".into(), tool_requirement: make_req("a", ToolCapabilityId::new()),
                estimated_duration_ms: 100, estimated_cost: 1.0, dependencies: vec![], is_critical: true },
            TaskNode { id: "b".into(), description: "B".into(), tool_requirement: make_req("b", ToolCapabilityId::new()),
                estimated_duration_ms: 200, estimated_cost: 2.0, dependencies: vec!["a".into()], is_critical: true },
        ];
        let graph = TaskGraph::new(nodes);
        assert_eq!(graph.entry_points, vec!["a"]);
        assert_eq!(graph.exit_points, vec!["b"]);
    }

    #[test]
    fn test_critical_path() {
        let nodes = vec![
            TaskNode { id: "a".into(), description: "A".into(), tool_requirement: make_req("a", ToolCapabilityId::new()),
                estimated_duration_ms: 100, estimated_cost: 1.0, dependencies: vec![], is_critical: true },
            TaskNode { id: "b".into(), description: "B".into(), tool_requirement: make_req("b", ToolCapabilityId::new()),
                estimated_duration_ms: 200, estimated_cost: 2.0, dependencies: vec!["a".into()], is_critical: true },
            TaskNode { id: "c".into(), description: "C".into(), tool_requirement: make_req("c", ToolCapabilityId::new()),
                estimated_duration_ms: 50, estimated_cost: 0.5, dependencies: vec!["a".into()], is_critical: false },
        ];
        let graph = TaskGraph::new(nodes);
        assert_eq!(graph.critical_path_duration(), 300);
    }

    #[test]
    fn test_topological_order() {
        let nodes = vec![
            TaskNode { id: "a".into(), description: "A".into(), tool_requirement: make_req("a", ToolCapabilityId::new()),
                estimated_duration_ms: 1, estimated_cost: 0.0, dependencies: vec![], is_critical: true },
            TaskNode { id: "b".into(), description: "B".into(), tool_requirement: make_req("b", ToolCapabilityId::new()),
                estimated_duration_ms: 1, estimated_cost: 0.0, dependencies: vec!["a".into()], is_critical: true },
        ];
        let graph = TaskGraph::new(nodes);
        let order = graph.topological_order();
        assert_eq!(order, vec!["a", "b"]);
    }

    #[test]
    fn test_task_graph_builder_linear() {
        let builder = TaskGraphBuilder::new();
        let reqs = vec![make_req("r1", ToolCapabilityId::new()), make_req("r2", ToolCapabilityId::new())];
        let graph = builder.build_linear(make_goal_id(1), reqs);
        assert_eq!(graph.nodes.len(), 2);
        assert!(builder.validate(&graph).is_ok());
    }

    #[test]
    fn test_task_graph_builder_parallel() {
        let builder = TaskGraphBuilder::new();
        let reqs = vec![
            vec![make_req("r1", ToolCapabilityId::new())],
            vec![make_req("r2", ToolCapabilityId::new()), make_req("r3", ToolCapabilityId::new())],
        ];
        let graph = builder.build_parallel(make_goal_id(1), reqs);
        assert_eq!(graph.nodes.len(), 3);
        assert!(builder.validate(&graph).is_ok());
    }

    #[test]
    fn test_plan_optimizer() {
        let opt = PlanOptimizer::new();
        let nodes = vec![
            TaskNode { id: "a".into(), description: "A".into(), tool_requirement: make_req("a", ToolCapabilityId::new()),
                estimated_duration_ms: 1000, estimated_cost: 10.0, dependencies: vec![], is_critical: true },
            TaskNode { id: "b".into(), description: "B".into(), tool_requirement: make_req("b", ToolCapabilityId::new()),
                estimated_duration_ms: 500, estimated_cost: 5.0, dependencies: vec!["a".into()], is_critical: false },
        ];
        let graph = TaskGraph::new(nodes);
        let optimized = opt.optimize_duration(&graph);
        assert_eq!(optimized.nodes[0].estimated_duration_ms, 1000);
        assert_eq!(optimized.nodes[1].estimated_duration_ms, 250);
    }

    #[test]
    fn test_alternatives_generation() {
        let opt = PlanOptimizer::new();
        let nodes = vec![
            TaskNode { id: "a".into(), description: "A".into(), tool_requirement: make_req("a", ToolCapabilityId::new()),
                estimated_duration_ms: 100, estimated_cost: 1.0, dependencies: vec![], is_critical: true },
        ];
        let graph = TaskGraph::new(nodes);
        let alts = opt.generate_alternatives(&graph, 3);
        assert_eq!(alts.len(), 3);
    }

    #[test]
    fn test_tool_selector_best_match() {
        let sel = ToolSelector::new();
        use brain_core::ids::ToolId;
        let candidates = vec![
            ToolCandidate::new(ToolId::new(), "provider_a", ToolCapabilityId::new(), Confidence::new(0.5)),
            ToolCandidate::new(ToolId::new(), "provider_b", ToolCapabilityId::new(), Confidence::new(0.9)),
        ];
        let best = sel.best_match(&candidates);
        assert!(best.is_some());
    }

    #[test]
    fn test_plan_validation_empty() {
        let planner = TaskGraphBuilder::new();
        let graph = TaskGraph::new(vec![]);
        let result = planner.validate(&graph);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_planner_create_plan() {
        let registry = Arc::new(MockToolRegistry::new());
        let planner = Planner::new(registry);
        let goal = make_goal_id(1);
        let reqs = vec![make_req("r1", ToolCapabilityId::new())];
        let budget = brain_core::budget::CognitiveBudget::default();
        let plan = planner.create_plan(goal, reqs, &budget).await.unwrap();
        assert_eq!(plan.tool_requirements.len(), 1);
    }

    #[test]
    fn test_selector_scoring() {
        let sel = ToolSelector::new();
        let candidate = ToolCandidate::new(
            brain_core::ids::ToolId::new(),
            "test_provider",
            ToolCapabilityId::new(),
            Confidence::new(0.8),
        );
        let req = make_req("test", ToolCapabilityId::new());
        let score = sel.score_match(&candidate, &req);
        assert!(score > 0.0);
        assert!(score <= 1.0);
    }
}
