use async_trait::async_trait;
use execution_core::*;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::error::{PlannerError, PlannerResult};

// ── DefaultExecutionPlanner ─────────────────────────────────

#[derive(Debug, Clone)]
pub struct DefaultExecutionPlanner {
    registry: Arc<dyn ToolRegistry>,
    resolver: Arc<dyn ToolResolver>,
    sandbox: Arc<dyn SandboxEnforcer>,
    max_batch_size: usize,
}

impl DefaultExecutionPlanner {
    pub fn new(
        registry: Arc<dyn ToolRegistry>,
        resolver: Arc<dyn ToolResolver>,
        sandbox: Arc<dyn SandboxEnforcer>,
    ) -> Self {
        Self {
            registry,
            resolver,
            sandbox,
            max_batch_size: 100,
        }
    }

    pub fn with_max_batch_size(mut self, size: usize) -> Self {
        self.max_batch_size = size;
        self
    }

    pub fn max_batch_size(&self) -> usize {
        self.max_batch_size
    }

    pub fn registry(&self) -> &Arc<dyn ToolRegistry> {
        &self.registry
    }

    pub fn resolver(&self) -> &Arc<dyn ToolResolver> {
        &self.resolver
    }

    pub fn sandbox(&self) -> &Arc<dyn SandboxEnforcer> {
        &self.sandbox
    }
}

#[async_trait]
impl ExecutionPlanner for DefaultExecutionPlanner {
    async fn plan(&self, request: ExecutionRequest) -> ExecutionResult<ExecutionPlan> {
        let binding = self
            .resolver
            .resolve_capability(&request.capability_id)
            .await?;

        let sandbox_profile = self.sandbox.resolve_profile("default").await?;

        let routing_rules = vec![RoutingRule {
            condition: "always".into(),
            destinations: vec![RouteDestination::Caller, RouteDestination::Memory],
        }];

        let budget = request.budget.clone();
        let priority = request.priority;
        let permissions = request.context.session.permissions.clone();

        let plan = ExecutionPlan {
            id: ExecutionId::new(),
            request,
            binding,
            sandbox_profile,
            budget,
            priority,
            permissions,
            routing_rules,
            rollback_plan: None,
            state: ExecutionState::Planned,
            created_at: memory_core::Timestamp::now(),
        };

        self.sandbox.validate(&plan).await?;

        Ok(plan)
    }

    async fn plan_batch(
        &self,
        requests: Vec<ExecutionRequest>,
    ) -> ExecutionResult<Vec<ExecutionPlan>> {
        if requests.len() > self.max_batch_size {
            return Err(ExecutionError::ConfigurationError(format!(
                "batch too large: {} > {}",
                requests.len(),
                self.max_batch_size
            )));
        }

        let mut plans = Vec::with_capacity(requests.len());
        for req in requests {
            let expanded = self.expand_requirements(req).await?;
            for sub_req in expanded {
                plans.push(self.plan(sub_req).await?);
            }
        }

        Ok(plans)
    }

    async fn expand_requirements(
        &self,
        request: ExecutionRequest,
    ) -> ExecutionResult<Vec<ExecutionRequest>> {
        if request.capability_id.contains('+') {
            let capabilities: Vec<&str> = request.capability_id.split('+').collect();
            let mut requests = Vec::with_capacity(capabilities.len());
            for (i, cap) in capabilities.iter().enumerate() {
                let cap = cap.trim();
                if cap.is_empty() {
                    return Err(ExecutionError::ConfigurationError(
                        "empty capability in compound requirement".into(),
                    ));
                }
                let mut sub = request.clone();
                sub.capability_id = cap.to_string();
                sub.requirement_id = format!("{}-{}", request.requirement_id, i + 1);
                requests.push(sub);
            }
            Ok(requests)
        } else {
            Ok(vec![request])
        }
    }

    async fn build_dependency_graph(
        &self,
        plans: &[ExecutionPlan],
    ) -> ExecutionResult<DependencyGraph> {
        let nodes = plans.to_vec();
        let mut edges = Vec::new();

        for i in 0..plans.len() {
            for j in 0..plans.len() {
                if i == j {
                    continue;
                }

                let dep_cap = &plans[j].request.capability_id;
                let dep_req = &plans[j].request.requirement_id;

                for (input_key, input_val) in &plans[i].request.inputs {
                    if input_key == dep_cap
                        || input_val == dep_cap
                        || input_key == dep_req
                        || input_val == dep_req
                    {
                        edges.push(DependencyEdge {
                            from_id: plans[j].id,
                            to_id: plans[i].id,
                            edge_type: DependencyType::DataFlow,
                        });
                    }
                }
            }
        }

        if detect_cycle(&nodes, &edges) {
            return Err(ExecutionError::ConfigurationError(
                "circular dependency detected in execution plans".into(),
            ));
        }

        Ok(DependencyGraph { nodes, edges })
    }
}

// ── BatchPlanner ────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct BatchPlanner {
    inner: DefaultExecutionPlanner,
    max_batch_size: usize,
}

impl BatchPlanner {
    pub fn new(planner: DefaultExecutionPlanner) -> Self {
        let max_batch_size = planner.max_batch_size();
        Self {
            inner: planner,
            max_batch_size,
        }
    }

    pub fn with_max_batch_size(mut self, size: usize) -> Self {
        self.max_batch_size = size;
        self
    }

    pub fn max_batch_size(&self) -> usize {
        self.max_batch_size
    }

    pub fn inner(&self) -> &DefaultExecutionPlanner {
        &self.inner
    }

    pub async fn plan_batch(
        &self,
        requests: Vec<ExecutionRequest>,
    ) -> PlannerResult<Vec<ExecutionPlan>> {
        if requests.len() > self.max_batch_size {
            return Err(PlannerError::BatchTooLarge {
                count: requests.len(),
                max: self.max_batch_size,
            });
        }

        let mut plans = Vec::with_capacity(requests.len());
        for req in requests {
            let expanded = self.inner.expand_requirements(req).await?;
            for sub_req in expanded {
                plans.push(self.inner.plan(sub_req).await?);
            }
        }

        Ok(plans)
    }

    pub async fn identify_parallel_groups(
        &self,
        plans: &[ExecutionPlan],
    ) -> PlannerResult<Vec<Vec<ExecutionPlan>>> {
        let graph = self.inner.build_dependency_graph(plans).await?;

        if graph.edges.is_empty() {
            return Ok(vec![plans.to_vec()]);
        }

        let mut in_degree: HashMap<ExecutionId, usize> = HashMap::new();
        let mut adjacency: HashMap<ExecutionId, Vec<ExecutionId>> = HashMap::new();

        for node in &graph.nodes {
            in_degree.entry(node.id).or_insert(0);
            adjacency.entry(node.id).or_default();
        }

        for edge in &graph.edges {
            adjacency.entry(edge.from_id).or_default().push(edge.to_id);
            *in_degree.entry(edge.to_id).or_insert(0) += 1;
        }

        let mut levels: Vec<Vec<ExecutionId>> = Vec::new();
        let mut current: Vec<ExecutionId> = in_degree
            .iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(&id, _)| id)
            .collect();

        while !current.is_empty() {
            levels.push(current.clone());
            let mut next = Vec::new();
            for id in &current {
                if let Some(neighbors) = adjacency.get(id) {
                    for nid in neighbors {
                        if let Some(deg) = in_degree.get_mut(nid) {
                            *deg -= 1;
                            if *deg == 0 {
                                next.push(*nid);
                            }
                        }
                    }
                }
            }
            current = next;
        }

        let plan_map: HashMap<ExecutionId, ExecutionPlan> =
            graph.nodes.into_iter().map(|p| (p.id, p)).collect();

        Ok(levels
            .into_iter()
            .map(|level| {
                level
                    .into_iter()
                    .filter_map(|id| plan_map.get(&id).cloned())
                    .collect()
            })
            .filter(|g: &Vec<ExecutionPlan>| !g.is_empty())
            .collect())
    }
}

// ── Cycle Detection ─────────────────────────────────────────

fn detect_cycle(nodes: &[ExecutionPlan], edges: &[DependencyEdge]) -> bool {
    let all_ids: HashSet<ExecutionId> = nodes.iter().map(|n| n.id).collect();

    let mut adjacency: HashMap<ExecutionId, Vec<ExecutionId>> = HashMap::new();
    for edge in edges {
        adjacency.entry(edge.from_id).or_default().push(edge.to_id);
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Color {
        White,
        Gray,
        Black,
    }

    let mut color: HashMap<ExecutionId, Color> =
        all_ids.iter().map(|id| (*id, Color::White)).collect();

    fn dfs(
        id: ExecutionId,
        adj: &HashMap<ExecutionId, Vec<ExecutionId>>,
        color: &mut HashMap<ExecutionId, Color>,
    ) -> bool {
        color.insert(id, Color::Gray);
        if let Some(neighbors) = adj.get(&id) {
            for &nid in neighbors {
                match color.get(&nid).copied().unwrap_or(Color::White) {
                    Color::Gray => return true,
                    Color::White => {
                        if dfs(nid, adj, color) {
                            return true;
                        }
                    }
                    Color::Black => {}
                }
            }
        }
        color.insert(id, Color::Black);
        false
    }

    let ids: Vec<ExecutionId> = all_ids.into_iter().collect();
    for id in &ids {
        if color.get(id) == Some(&Color::White) && dfs(*id, &adjacency, &mut color) {
            return true;
        }
    }

    false
}

// ── Tests ───────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use execution_registry::{DefaultToolResolver, InMemoryToolRegistry};
    use execution_sandbox::DefaultSandboxEnforcer;
    use std::collections::HashMap;

    fn make_subprocess_binding(binary: &str) -> ToolBinding {
        ToolBinding::Subprocess {
            binary: binary.into(),
            args: Vec::new(),
            env: HashMap::new(),
            working_dir: None,
            allowed_paths: Vec::new(),
            denied_binaries: Vec::new(),
        }
    }

    fn make_capability(id: &str, name: &str) -> ExecutionCapability {
        ExecutionCapability {
            id: id.into(),
            name: name.into(),
            description: String::new(),
            input_schema: HashMap::new(),
            output_schema: HashMap::new(),
            required_permissions: Vec::new(),
        }
    }

    fn make_request(capability_id: &str) -> ExecutionRequest {
        ExecutionRequest {
            requirement_id: format!("req-{}", capability_id),
            capability_id: capability_id.into(),
            inputs: HashMap::new(),
            context: ExecutionContext {
                session: ExecutionSession {
                    session_id: "test-session".into(),
                    user_id: "test-user".into(),
                    roles: vec!["admin".into()],
                    permissions: ExecutionPermissions::default(),
                },
                trace_id: "trace-1".into(),
                span_id: "span-1".into(),
                originating_goal: None,
                originating_plan: None,
            },
            budget: ExecutionBudget::default(),
            priority: ExecutionPriority::default(),
            retry_policy: RetryPolicy::default(),
        }
    }

    async fn setup_planner() -> DefaultExecutionPlanner {
        let registry = Arc::new(InMemoryToolRegistry::new());
        let binding = make_subprocess_binding("/usr/bin/echo");
        let caps = vec![make_capability("text.echo", "Echo Text")];
        registry.register(binding, caps).await.unwrap();

        let resolver: Arc<dyn ToolResolver> = Arc::new(DefaultToolResolver::new(registry.clone()));
        let sandbox: Arc<dyn SandboxEnforcer> = Arc::new(DefaultSandboxEnforcer::new());

        DefaultExecutionPlanner::new(registry, resolver, sandbox)
    }

    #[tokio::test]
    async fn test_plan_creates_valid_plan() {
        let planner = setup_planner().await;
        let request = make_request("text.echo");
        let plan = planner.plan(request).await.unwrap();

        assert_eq!(plan.state, ExecutionState::Planned);
        match &plan.binding {
            ToolBinding::Subprocess { binary, .. } => {
                assert_eq!(binary, "/usr/bin/echo");
            }
            _ => panic!("expected Subprocess binding"),
        }
        assert_eq!(plan.sandbox_profile.name, "default");
        assert_eq!(plan.routing_rules.len(), 1);
    }

    #[tokio::test]
    async fn test_plan_missing_capability_returns_error() {
        let planner = setup_planner().await;
        let request = make_request("nonexistent");
        let result = planner.plan(request).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_expand_requirements_single() {
        let planner = setup_planner().await;
        let request = make_request("text.echo");
        let expanded = planner.expand_requirements(request).await.unwrap();
        assert_eq!(expanded.len(), 1);
    }

    #[tokio::test]
    async fn test_expand_requirements_compound() {
        let planner = setup_planner().await;
        let mut request = make_request("file.read+file.write");
        request.requirement_id = "req".into();
        let expanded = planner.expand_requirements(request).await.unwrap();
        assert_eq!(expanded.len(), 2);
        assert_eq!(expanded[0].capability_id, "file.read");
        assert_eq!(expanded[1].capability_id, "file.write");
        assert_eq!(expanded[0].requirement_id, "req-1");
        assert_eq!(expanded[1].requirement_id, "req-2");
    }

    #[tokio::test]
    async fn test_plan_batch_multiple() {
        let planner = setup_planner().await;
        let requests: Vec<ExecutionRequest> = (0..5)
            .map(|i| {
                let mut req = make_request("text.echo");
                req.requirement_id = format!("req-{}", i);
                req
            })
            .collect();

        let plans = planner.plan_batch(requests).await.unwrap();
        assert_eq!(plans.len(), 5);
        let ids: HashSet<ExecutionId> = plans.iter().map(|p| p.id).collect();
        assert_eq!(ids.len(), 5);
    }

    #[tokio::test]
    async fn test_build_dependency_graph_no_deps() {
        let planner = setup_planner().await;
        let plan = planner.plan(make_request("text.echo")).await.unwrap();
        let graph = planner.build_dependency_graph(&[plan]).await.unwrap();
        assert_eq!(graph.nodes.len(), 1);
        assert!(graph.edges.is_empty());
    }

    #[tokio::test]
    async fn test_batch_planner_parallel_groups_no_deps() {
        let planner = setup_planner().await;
        let bp = BatchPlanner::new(planner.clone());
        let p1 = planner.plan(make_request("text.echo")).await.unwrap();
        let p2 = planner.plan(make_request("text.echo")).await.unwrap();

        let groups = bp.identify_parallel_groups(&[p1, p2]).await.unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].len(), 2);
    }

    #[tokio::test]
    async fn test_detect_cycle_no_cycle() {
        let planner = setup_planner().await;
        let p1 = planner.plan(make_request("text.echo")).await.unwrap();
        let p2 = planner.plan(make_request("text.echo")).await.unwrap();

        let graph = planner.build_dependency_graph(&[p1, p2]).await.unwrap();
        assert!(!detect_cycle(&graph.nodes, &graph.edges));
    }

    #[tokio::test]
    async fn test_planner_default_batch_size() {
        let planner = setup_planner().await;
        assert_eq!(planner.max_batch_size(), 100);
    }

    #[tokio::test]
    async fn test_planner_with_max_batch_size() {
        let planner = setup_planner().await.with_max_batch_size(10);
        assert_eq!(planner.max_batch_size(), 10);
    }

    #[tokio::test]
    async fn test_batch_planner_exceeds_max_size() {
        let planner = setup_planner().await.with_max_batch_size(1);
        let bp = BatchPlanner::new(planner);
        let requests = vec![make_request("text.echo"), make_request("text.echo")];
        let result = bp.plan_batch(requests).await;
        assert!(result.is_err());
        match result {
            Err(PlannerError::BatchTooLarge { count, max }) => {
                assert_eq!(count, 2);
                assert_eq!(max, 1);
            }
            _ => panic!("expected BatchTooLarge error"),
        }
    }
}
