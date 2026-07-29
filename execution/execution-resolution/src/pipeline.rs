use async_trait::async_trait;
use execution_core::*;
use memory_core::Timestamp;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use execution_adapter::DefaultAdapterGenerator;
use execution_cache::{DefaultLearningEngine, InMemoryCapabilityCache};
use execution_discovery::{DefaultExternalDiscoverer, DefaultLocalDiscoverer};
use execution_planner::DefaultExecutionPlanner;

#[derive(Debug)]
pub struct DefaultResolutionPipeline {
    _registry: Arc<dyn ToolRegistry>,
    local_discoverer: Arc<dyn CapabilityDiscoverer>,
    external_discoverer: Arc<dyn CapabilityDiscoverer>,
    adapter_generator: Arc<dyn AdapterGenerator>,
    cache: Arc<dyn CapabilityCache>,
    learning_engine: Arc<dyn LearningEngine>,
    sandbox: Arc<dyn SandboxEnforcer>,
    human_approval: Option<Arc<dyn HumanApproval>>,
    planner: Arc<DefaultExecutionPlanner>,
    reports: RwLock<HashMap<String, ResolutionReport>>,
    bypass: RwLock<HashMap<ResolutionStage, StageBypassState>>,
    stats: RwLock<HashMap<String, u64>>,
}

impl DefaultResolutionPipeline {
    pub fn new(
        registry: Arc<dyn ToolRegistry>,
        sandbox: Arc<dyn SandboxEnforcer>,
        human_approval: Option<Arc<dyn HumanApproval>>,
    ) -> Self {
        let local_discoverer = Arc::new(DefaultLocalDiscoverer::new());
        let external_discoverer = Arc::new(DefaultExternalDiscoverer::new());
        let adapter_generator = Arc::new(DefaultAdapterGenerator::new());
        let cache = Arc::new(InMemoryCapabilityCache::new());
        let learning_engine = Arc::new(DefaultLearningEngine::new());
        let resolver: Arc<dyn ToolResolver> = Arc::new(
            execution_registry::DefaultToolResolver::new(registry.clone()),
        );

        let planner = Arc::new(DefaultExecutionPlanner::new(
            registry.clone(),
            resolver,
            sandbox.clone(),
        ));

        let mut bypass = HashMap::new();
        bypass.insert(
            ResolutionStage::NativeRegistry,
            StageBypassState {
                stage: ResolutionStage::NativeRegistry,
                engaged: false,
                failure_count: 0,
                engaged_at: None,
                reason: None,
            },
        );

        Self {
            _registry: registry,
            local_discoverer,
            external_discoverer,
            adapter_generator,
            cache,
            learning_engine,
            sandbox,
            human_approval,
            planner,
            reports: RwLock::new(HashMap::new()),
            bypass: RwLock::new(bypass),
            stats: RwLock::new(HashMap::new()),
        }
    }

    pub fn with_cache(mut self, cache: Arc<dyn CapabilityCache>) -> Self {
        self.cache = cache;
        self
    }

    pub fn with_learning(mut self, learning: Arc<dyn LearningEngine>) -> Self {
        self.learning_engine = learning;
        self
    }

    pub fn with_local_discoverer(mut self, discoverer: Arc<dyn CapabilityDiscoverer>) -> Self {
        self.local_discoverer = discoverer;
        self
    }

    pub fn with_external_discoverer(mut self, discoverer: Arc<dyn CapabilityDiscoverer>) -> Self {
        self.external_discoverer = discoverer;
        self
    }

    pub fn with_adapter_generator(mut self, generator: Arc<dyn AdapterGenerator>) -> Self {
        self.adapter_generator = generator;
        self
    }

    fn bump_stat(&self, key: &str) {
        if let Ok(mut stats) = self.stats.try_write() {
            *stats.entry(key.to_string()).or_insert(0) += 1;
        }
    }

    fn store_report(&self, report: ResolutionReport) {
        if let Ok(mut reports) = self.reports.try_write() {
            reports.insert(report.capability_id.clone(), report);
        }
    }

    fn is_bypassed(&self, stage: &ResolutionStage) -> bool {
        if let Ok(bypass) = self.bypass.try_read() {
            if let Some(state) = bypass.get(stage) {
                return state.engaged;
            }
        }
        false
    }

    async fn stage_native_registry(
        &self,
        request: &ExecutionRequest,
    ) -> Result<ExecutionPlan, ExecutionError> {
        let start = std::time::Instant::now();

        let plan = self.planner.plan(request.clone()).await?;

        let duration = start.elapsed().as_millis() as u64;
        self.store_report(ResolutionReport {
            capability_id: request.capability_id.clone(),
            requirement_id: request.requirement_id.clone(),
            stage: ResolutionStage::NativeRegistry,
            success: true,
            origin: CapabilityOrigin::Native,
            provider: None,
            adapter: None,
            duration_ms: duration,
            error: None,
        });

        info!(
            capability = %request.capability_id,
            "resolved via native registry"
        );
        Ok(plan)
    }

    async fn stage_learned_cache(
        &self,
        request: &ExecutionRequest,
    ) -> Result<ExecutionPlan, ExecutionError> {
        let start = std::time::Instant::now();

        if let Some(entry) = self.cache.lookup(&request.capability_id).await? {
            if let Some(ref adapter) = entry.adapter {
                let binding = match adapter.adapter_type {
                    AdapterType::CliWrapper | AdapterType::BashScript => ToolBinding::Subprocess {
                        binary: adapter
                            .required_runtime
                            .clone()
                            .unwrap_or_else(|| "sh".into()),
                        args: vec!["-c".into(), adapter.template.clone()],
                        env: HashMap::new(),
                        working_dir: None,
                        allowed_paths: Vec::new(),
                        denied_binaries: Vec::new(),
                    },
                    AdapterType::PythonScript => ToolBinding::Subprocess {
                        binary: "python3".into(),
                        args: vec!["-c".into(), adapter.template.clone()],
                        env: HashMap::new(),
                        working_dir: None,
                        allowed_paths: Vec::new(),
                        denied_binaries: Vec::new(),
                    },
                    AdapterType::JavaScript => ToolBinding::Subprocess {
                        binary: "node".into(),
                        args: vec!["-e".into(), adapter.template.clone()],
                        env: HashMap::new(),
                        working_dir: None,
                        allowed_paths: Vec::new(),
                        denied_binaries: Vec::new(),
                    },
                    AdapterType::RestClient => ToolBinding::Subprocess {
                        binary: "curl".into(),
                        args: vec!["-s".into(), adapter.template.clone()],
                        env: HashMap::new(),
                        working_dir: None,
                        allowed_paths: Vec::new(),
                        denied_binaries: Vec::new(),
                    },
                    _ => ToolBinding::Subprocess {
                        binary: "sh".into(),
                        args: vec!["-c".into(), adapter.template.clone()],
                        env: HashMap::new(),
                        working_dir: None,
                        allowed_paths: Vec::new(),
                        denied_binaries: Vec::new(),
                    },
                };

                let plan = ExecutionPlan {
                    id: ExecutionId::new(),
                    request: request.clone(),
                    binding,
                    sandbox_profile: SandboxProfile::new(&adapter.sandbox_profile),
                    budget: request.budget.clone(),
                    priority: request.priority,
                    permissions: request.context.session.permissions.clone(),
                    routing_rules: vec![RoutingRule {
                        condition: "always".into(),
                        destinations: vec![RouteDestination::Caller, RouteDestination::Memory],
                    }],
                    rollback_plan: None,
                    state: ExecutionState::Planned,
                    created_at: memory_core::Timestamp::now(),
                };

                self.sandbox.validate(&plan).await?;

                let duration = start.elapsed().as_millis() as u64;
                self.store_report(ResolutionReport {
                    capability_id: request.capability_id.clone(),
                    requirement_id: request.requirement_id.clone(),
                    stage: ResolutionStage::LearnedCache,
                    success: true,
                    origin: CapabilityOrigin::Learned,
                    provider: Some(entry.provider_metadata.clone()),
                    adapter: Some(adapter.clone()),
                    duration_ms: duration,
                    error: None,
                });

                info!(
                    capability = %request.capability_id,
                    provider = %entry.provider_metadata.name,
                    "resolved via learned cache"
                );
                return Ok(plan);
            }
        }

        Err(ExecutionError::NotFound(format!(
            "learned cache miss for {}",
            request.capability_id
        )))
    }

    async fn discover_and_generate(
        &self,
        request: &ExecutionRequest,
        discoverer: &dyn CapabilityDiscoverer,
        stage: ResolutionStage,
        origin: CapabilityOrigin,
    ) -> Result<ExecutionPlan, ExecutionError> {
        let start = std::time::Instant::now();

        let query = DiscoveryQuery {
            capability_id: request.capability_id.clone(),
            capability_name: request.capability_id.clone(),
            keywords: vec![],
            categories: Vec::new(),
        };

        let discovered = if stage == ResolutionStage::LocalDiscovery {
            discoverer.discover_local(&query).await?
        } else {
            discoverer.discover_external(&query).await?
        };

        if discovered.is_empty() {
            let duration = start.elapsed().as_millis() as u64;
            self.store_report(ResolutionReport {
                capability_id: request.capability_id.clone(),
                requirement_id: request.requirement_id.clone(),
                stage,
                success: false,
                origin,
                provider: None,
                adapter: None,
                duration_ms: duration,
                error: Some("no providers found".into()),
            });
            return Err(ExecutionError::NotFound(format!(
                "no {stage} providers for {}",
                request.capability_id
            )));
        }

        for cap in &discovered {
            if self.adapter_generator.can_synthesize(cap).await {
                let synthesis_request = CapabilitySynthesisRequest {
                    capability_id: cap.id.clone(),
                    capability_name: cap.name.clone(),
                    description: cap.description.clone(),
                    provider: cap.provider.clone(),
                    input_schema: cap.input_schema.clone(),
                    output_schema: cap.output_schema.clone(),
                    preferred_adapter_type: cap.adapter_template.as_ref().map(|t| t.adapter_type),
                };

                let adapter = self.adapter_generator.generate(&synthesis_request).await?;

                let binding = ToolBinding::Subprocess {
                    binary: adapter
                        .required_runtime
                        .clone()
                        .unwrap_or_else(|| cap.provider.name.clone()),
                    args: vec!["-c".into(), adapter.template.clone()],
                    env: HashMap::new(),
                    working_dir: None,
                    allowed_paths: Vec::new(),
                    denied_binaries: Vec::new(),
                };

                let plan = ExecutionPlan {
                    id: ExecutionId::new(),
                    request: request.clone(),
                    binding,
                    sandbox_profile: SandboxProfile::new(&adapter.sandbox_profile),
                    budget: request.budget.clone(),
                    priority: request.priority,
                    permissions: request.context.session.permissions.clone(),
                    routing_rules: vec![RoutingRule {
                        condition: "always".into(),
                        destinations: vec![RouteDestination::Caller, RouteDestination::Memory],
                    }],
                    rollback_plan: None,
                    state: ExecutionState::Planned,
                    created_at: memory_core::Timestamp::now(),
                };

                self.sandbox.validate(&plan).await?;

                let cache_entry = CapabilityCacheEntry {
                    capability_id: request.capability_id.clone(),
                    origin,
                    provider_metadata: cap.provider.clone(),
                    trust: cap.trust.clone(),
                    supported_parameters: cap.supported_parameters.clone(),
                    required_permissions: cap.required_permissions.clone(),
                    adapter: Some(adapter.clone()),
                    created_at: Timestamp::now(),
                    last_accessed: Timestamp::now(),
                    access_count: 0,
                };
                let _ = self.cache.store(cache_entry).await;

                let duration = start.elapsed().as_millis() as u64;
                self.store_report(ResolutionReport {
                    capability_id: request.capability_id.clone(),
                    requirement_id: request.requirement_id.clone(),
                    stage,
                    success: true,
                    origin,
                    provider: Some(cap.provider.clone()),
                    adapter: Some(adapter),
                    duration_ms: duration,
                    error: None,
                });

                info!(
                    capability = %request.capability_id,
                    provider = %cap.provider.name,
                    ?stage,
                    "resolved via discovery and adapter generation"
                );
                return Ok(plan);
            }
        }

        let duration = start.elapsed().as_millis() as u64;
        self.store_report(ResolutionReport {
            capability_id: request.capability_id.clone(),
            requirement_id: request.requirement_id.clone(),
            stage,
            success: false,
            origin,
            provider: None,
            adapter: None,
            duration_ms: duration,
            error: Some("no synthesizable provider found".into()),
        });
        Err(ExecutionError::NotFound(format!(
            "no synthesizable provider in {stage} for {}",
            request.capability_id
        )))
    }

    async fn stage_capability_synthesis(
        &self,
        request: &ExecutionRequest,
    ) -> Result<ExecutionPlan, ExecutionError> {
        let start = std::time::Instant::now();

        let query = DiscoveryQuery {
            capability_id: request.capability_id.clone(),
            capability_name: request.capability_id.clone(),
            keywords: vec![],
            categories: Vec::new(),
        };

        let local_results = self.local_discoverer.discover_local(&query).await?;
        let external_results = self.external_discoverer.discover_external(&query).await?;

        let all_candidates: Vec<&DiscoveredCapability> = local_results
            .iter()
            .chain(external_results.iter())
            .collect();

        if all_candidates.is_empty() {
            let duration = start.elapsed().as_millis() as u64;
            self.store_report(ResolutionReport {
                capability_id: request.capability_id.clone(),
                requirement_id: request.requirement_id.clone(),
                stage: ResolutionStage::CapabilitySynthesis,
                success: false,
                origin: CapabilityOrigin::Generated,
                provider: None,
                adapter: None,
                duration_ms: duration,
                error: Some("no candidates available for synthesis".into()),
            });
            return Err(ExecutionError::NotFound(format!(
                "no candidates available for synthesis: {}",
                request.capability_id
            )));
        }

        for cap in all_candidates {
            if cap.trust.score < 0.3 {
                warn!(
                    capability = %cap.id,
                    trust = cap.trust.score,
                    "skipping low-trust candidate for synthesis"
                );
                continue;
            }

            if !cap.compatible {
                continue;
            }

            let synthesis_request = CapabilitySynthesisRequest {
                capability_id: cap.id.clone(),
                capability_name: cap.name.clone(),
                description: cap.description.clone(),
                provider: cap.provider.clone(),
                input_schema: cap.input_schema.clone(),
                output_schema: cap.output_schema.clone(),
                preferred_adapter_type: cap.adapter_template.as_ref().map(|t| t.adapter_type),
            };

            match self.adapter_generator.generate(&synthesis_request).await {
                Ok(adapter) => {
                    let binding = ToolBinding::Subprocess {
                        binary: adapter
                            .required_runtime
                            .clone()
                            .unwrap_or_else(|| cap.provider.name.clone()),
                        args: vec!["-c".into(), adapter.template.clone()],
                        env: HashMap::new(),
                        working_dir: None,
                        allowed_paths: Vec::new(),
                        denied_binaries: Vec::new(),
                    };

                    let plan = ExecutionPlan {
                        id: ExecutionId::new(),
                        request: request.clone(),
                        binding,
                        sandbox_profile: SandboxProfile::new(&adapter.sandbox_profile),
                        budget: request.budget.clone(),
                        priority: request.priority,
                        permissions: request.context.session.permissions.clone(),
                        routing_rules: vec![RoutingRule {
                            condition: "always".into(),
                            destinations: vec![RouteDestination::Caller, RouteDestination::Memory],
                        }],
                        rollback_plan: None,
                        state: ExecutionState::Planned,
                        created_at: memory_core::Timestamp::now(),
                    };

                    self.sandbox.validate(&plan).await?;

                    let cache_entry = CapabilityCacheEntry {
                        capability_id: request.capability_id.clone(),
                        origin: CapabilityOrigin::Generated,
                        provider_metadata: cap.provider.clone(),
                        trust: cap.trust.clone(),
                        supported_parameters: cap.supported_parameters.clone(),
                        required_permissions: cap.required_permissions.clone(),
                        adapter: Some(adapter.clone()),
                        created_at: Timestamp::now(),
                        last_accessed: Timestamp::now(),
                        access_count: 0,
                    };
                    let _ = self.cache.store(cache_entry).await;

                    let duration = start.elapsed().as_millis() as u64;
                    self.store_report(ResolutionReport {
                        capability_id: request.capability_id.clone(),
                        requirement_id: request.requirement_id.clone(),
                        stage: ResolutionStage::CapabilitySynthesis,
                        success: true,
                        origin: CapabilityOrigin::Generated,
                        provider: Some(cap.provider.clone()),
                        adapter: Some(adapter),
                        duration_ms: duration,
                        error: None,
                    });

                    info!(
                        capability = %request.capability_id,
                        provider = %cap.provider.name,
                        "resolved via capability synthesis"
                    );
                    return Ok(plan);
                }
                Err(e) => {
                    debug!(
                        capability = %cap.id,
                        error = %e,
                        "synthesis failed for candidate"
                    );
                    continue;
                }
            }
        }

        let duration = start.elapsed().as_millis() as u64;
        self.store_report(ResolutionReport {
            capability_id: request.capability_id.clone(),
            requirement_id: request.requirement_id.clone(),
            stage: ResolutionStage::CapabilitySynthesis,
            success: false,
            origin: CapabilityOrigin::Generated,
            provider: None,
            adapter: None,
            duration_ms: duration,
            error: Some("all synthesis candidates failed".into()),
        });
        Err(ExecutionError::AdapterGenerationFailed {
            capability: request.capability_id.clone(),
            detail: "all synthesis candidates failed".into(),
        })
    }

    async fn stage_human_approval(
        &self,
        request: &ExecutionRequest,
    ) -> Result<ExecutionPlan, ExecutionError> {
        let approval = self
            .human_approval
            .as_ref()
            .ok_or_else(|| ExecutionError::HumanApprovalRequired(request.capability_id.clone()))?;

        let dummy_provider = ProviderMetadata::new("human_approval", "approval", "manual");
        let dummy_trust = TrustScore::new(CapabilityOrigin::Generated);

        let approved = approval
            .request_approval(
                &request.capability_id,
                &format!(
                    "Human approval needed for capability: {}",
                    request.capability_id
                ),
                &dummy_provider,
                &dummy_trust,
            )
            .await?;

        if approved {
            info!(
                capability = %request.capability_id,
                "human approval granted"
            );
            Err(ExecutionError::NotFound(format!(
                "human approved but no provider for {}",
                request.capability_id
            )))
        } else {
            Err(ExecutionError::HumanApprovalDenied(
                request.capability_id.clone(),
            ))
        }
    }
}

#[async_trait]
impl ResolutionPipeline for DefaultResolutionPipeline {
    async fn resolve(
        &self,
        request: ExecutionRequest,
    ) -> execution_core::ExecutionResult<ExecutionPlan> {
        let stages = [
            (ResolutionStage::NativeRegistry, "native_registry" as &str),
            (ResolutionStage::LearnedCache, "learned_cache"),
            (ResolutionStage::LocalDiscovery, "local_discovery"),
            (ResolutionStage::ExternalDiscovery, "external_discovery"),
            (ResolutionStage::CapabilitySynthesis, "capability_synthesis"),
            (ResolutionStage::HumanApproval, "human_approval"),
        ];

        for (stage, _name) in &stages {
            if self.is_bypassed(stage) {
                debug!(?stage, "stage is bypassed, skipping");
                continue;
            }

            let result = match stage {
                ResolutionStage::NativeRegistry => {
                    self.bump_stat("native_registry_attempts");
                    self.stage_native_registry(&request).await
                }
                ResolutionStage::LearnedCache => {
                    self.bump_stat("learned_cache_attempts");
                    self.stage_learned_cache(&request).await
                }
                ResolutionStage::LocalDiscovery => {
                    self.bump_stat("local_discovery_attempts");
                    self.discover_and_generate(
                        &request,
                        &*self.local_discoverer,
                        *stage,
                        CapabilityOrigin::DiscoveredLocal,
                    )
                    .await
                }
                ResolutionStage::ExternalDiscovery => {
                    self.bump_stat("external_discovery_attempts");
                    self.discover_and_generate(
                        &request,
                        &*self.external_discoverer,
                        *stage,
                        CapabilityOrigin::DiscoveredExternal,
                    )
                    .await
                }
                ResolutionStage::CapabilitySynthesis => {
                    self.bump_stat("synthesis_attempts");
                    self.stage_capability_synthesis(&request).await
                }
                ResolutionStage::HumanApproval => {
                    self.bump_stat("human_approval_attempts");
                    self.stage_human_approval(&request).await
                }
                _ => {
                    return Err(ExecutionError::ConfigurationError(format!(
                        "unknown resolution stage: {stage}"
                    )))
                }
            };

            match result {
                Ok(plan) => {
                    self.bump_stat(&format!("{}_success", stage));
                    return Ok(plan);
                }
                Err(e) => {
                    debug!(?stage, error = %e, "stage failed, trying next");
                    self.bump_stat(&format!("{}_failures", stage));

                    if matches!(e, ExecutionError::HumanApprovalDenied(_)) {
                        return Err(e);
                    }
                }
            }
        }

        warn!(
            capability = %request.capability_id,
            "resolution pipeline exhausted all stages"
        );
        Err(ExecutionError::ResolutionExhausted {
            capability: request.capability_id.clone(),
        })
    }

    async fn resolution_report(
        &self,
        capability_id: &str,
    ) -> execution_core::ExecutionResult<Option<ResolutionReport>> {
        let reports = self.reports.read().await;
        Ok(reports.get(capability_id).cloned())
    }

    async fn report_execution(
        &self,
        capability_id: &str,
        _plan: &ExecutionPlan,
        result: &execution_core::types::ExecutionResult,
    ) -> execution_core::ExecutionResult<()> {
        let success = result.state == ExecutionState::Completed;
        let failure_mode = result.error.clone();

        self.learning_engine
            .record_execution(
                capability_id,
                success,
                result.metrics.wall_clock_ms,
                failure_mode,
            )
            .await?;

        if success {
            self.bump_stat("successful_executions");
        } else {
            self.bump_stat("failed_executions");
        }

        Ok(())
    }

    async fn invalidate_cache(&self, capability_id: &str) -> execution_core::ExecutionResult<()> {
        self.cache.invalidate(capability_id).await?;
        Ok(())
    }

    fn pipeline_stats(&self) -> HashMap<String, u64> {
        self.stats.try_read().map(|s| s.clone()).unwrap_or_default()
    }

    fn resolution_order(&self) -> Vec<ResolutionStage> {
        vec![
            ResolutionStage::NativeRegistry,
            ResolutionStage::LearnedCache,
            ResolutionStage::LocalDiscovery,
            ResolutionStage::ExternalDiscovery,
            ResolutionStage::CapabilitySynthesis,
            ResolutionStage::HumanApproval,
        ]
    }

    fn set_bypass(&self, stage: ResolutionStage, engage: bool) {
        if let Ok(mut bypass) = self.bypass.try_write() {
            let entry = bypass.entry(stage).or_insert_with(|| StageBypassState {
                stage,
                engaged: false,
                failure_count: 0,
                engaged_at: None,
                reason: None,
            });
            entry.engaged = engage;
            if engage {
                entry.engaged_at = Some(Timestamp::now());
                entry.reason = Some("manually bypassed".into());
            } else {
                entry.engaged_at = None;
                entry.reason = None;
            }
        }
    }

    fn bypass_state(&self, stage: &ResolutionStage) -> Option<StageBypassState> {
        self.bypass
            .try_read()
            .ok()
            .and_then(|b| b.get(stage).cloned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use execution_registry::InMemoryToolRegistry;
    use execution_sandbox::DefaultSandboxEnforcer;
    use std::collections::HashMap;

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

    fn setup_pipeline() -> DefaultResolutionPipeline {
        let registry = Arc::new(InMemoryToolRegistry::new());
        let sandbox = Arc::new(DefaultSandboxEnforcer::new());
        DefaultResolutionPipeline::new(registry, sandbox, None)
    }

    #[tokio::test]
    async fn test_resolution_order() {
        let pipeline = setup_pipeline();
        let order = pipeline.resolution_order();
        assert_eq!(order[0], ResolutionStage::NativeRegistry);
        assert_eq!(order[1], ResolutionStage::LearnedCache);
        assert_eq!(order[2], ResolutionStage::LocalDiscovery);
        assert_eq!(order[3], ResolutionStage::ExternalDiscovery);
        assert_eq!(order[4], ResolutionStage::CapabilitySynthesis);
        assert_eq!(order[5], ResolutionStage::HumanApproval);
    }

    #[tokio::test]
    async fn test_resolve_unknown_capability_exhausts_pipeline() {
        let pipeline = setup_pipeline();
        let request = make_request("nonexistent.capability");
        let result = pipeline.resolve(request).await;
        assert!(result.is_err());
        match result {
            Err(ExecutionError::ResolutionExhausted { capability }) => {
                assert_eq!(capability, "nonexistent.capability");
            }
            _ => panic!("expected ResolutionExhausted error"),
        }
    }

    #[tokio::test]
    async fn test_bypass_mechanism() {
        let pipeline = setup_pipeline();
        pipeline.set_bypass(ResolutionStage::NativeRegistry, true);
        let state = pipeline.bypass_state(&ResolutionStage::NativeRegistry);
        assert!(state.is_some());
        assert!(state.unwrap().engaged);
    }

    #[tokio::test]
    async fn test_pipeline_stats() {
        let pipeline = setup_pipeline();
        let request = make_request("unknown.xyz");
        let _ = pipeline.resolve(request).await;
        let stats = pipeline.pipeline_stats();
        assert!(stats.contains_key("native_registry_attempts"));
        assert!(stats.contains_key("native_registry_failures"));
    }

    #[tokio::test]
    async fn test_report_execution() {
        let pipeline = setup_pipeline();
        let plan = ExecutionPlan {
            id: ExecutionId::new(),
            request: make_request("test.cap"),
            binding: ToolBinding::Subprocess {
                binary: "/bin/true".into(),
                args: vec![],
                env: HashMap::new(),
                working_dir: None,
                allowed_paths: Vec::new(),
                denied_binaries: Vec::new(),
            },
            sandbox_profile: SandboxProfile::new("default"),
            budget: ExecutionBudget::default(),
            priority: ExecutionPriority::default(),
            permissions: ExecutionPermissions::default(),
            routing_rules: Vec::new(),
            rollback_plan: None,
            state: ExecutionState::Completed,
            created_at: Timestamp::now(),
        };
        let result = execution_core::types::ExecutionResult {
            execution_id: plan.id,
            state: ExecutionState::Completed,
            exit_code: Some(0),
            stdout: Vec::new(),
            stderr: Vec::new(),
            parsed_output: None,
            artifacts: Vec::new(),
            metrics: ExecutionMetrics::default(),
            error: None,
            routing_results: Vec::new(),
            started_at: Timestamp::now(),
            completed_at: Timestamp::now(),
        };

        pipeline
            .report_execution("test.cap", &plan, &result)
            .await
            .unwrap();

        let learned = pipeline
            .learning_engine
            .get_learned("test.cap")
            .await
            .unwrap();
        assert!(learned.is_some());
        assert_eq!(learned.unwrap().execution_count, 1);
    }

    #[tokio::test]
    async fn test_invalidate_cache() {
        let pipeline = setup_pipeline();
        let entry = CapabilityCacheEntry {
            capability_id: "test.cap".into(),
            origin: CapabilityOrigin::DiscoveredLocal,
            provider_metadata: ProviderMetadata::new("test", "cli", "/test"),
            trust: TrustScore::new(CapabilityOrigin::DiscoveredLocal),
            supported_parameters: HashMap::new(),
            required_permissions: Vec::new(),
            adapter: None,
            created_at: Timestamp::now(),
            last_accessed: Timestamp::now(),
            access_count: 0,
        };
        pipeline.cache.store(entry).await.unwrap();
        assert!(pipeline.cache.lookup("test.cap").await.unwrap().is_some());
        pipeline.invalidate_cache("test.cap").await.unwrap();
        assert!(pipeline.cache.lookup("test.cap").await.unwrap().is_none());
    }
}
