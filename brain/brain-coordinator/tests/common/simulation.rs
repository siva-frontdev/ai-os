use std::collections::VecDeque;
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, Mutex};

use brain_coordinator::cognitive_loop::CognitiveLoopService;
use brain_core::types::Decision;
use intelligence_coordinator::world_understanding::{
    StructuredWorldUpdate, WorldUnderstandingService,
};
use intelligence_core::traits::IntelligenceCoordinator;
use intelligence_core::types::{
    ConversationId, Embedding, IntelligenceStats, ModelId, ModelInput, ModelRequest, ModelResponse,
    RequestId, StreamChunk, TokenUsage,
};
use memory_core::Timestamp;
use memory_core::wm::{Entity, EntityLifecycle};
use memory_storage::wm_store::{InMemoryWorldModelStore, WorldModelStore};

use super::scenarios::ScenarioDay;
use futures::Stream;

const SECONDS_PER_DAY: u64 = 86_400;
const NANOS_PER_DAY: i64 = SECONDS_PER_DAY as i64 * 1_000_000_000;

/// Record of one simulated day's events and decisions.
#[derive(Debug, Clone)]
pub struct DayTrace {
    pub day: u64,
    pub observation: Option<String>,
    pub cycle_decision: Option<Decision>,
    pub tick_decision: Decision,
    pub context_entity_names: Vec<String>,
    pub context_summary: String,
    pub reflection_recorded: bool,
}

/// Simulation engine that runs multi-day scenarios against CognitiveLoopService.
///
/// Key design decisions:
///   - Uses a `ScriptedCoordinator` that returns pre-defined StructuredWorldUpdate
///     responses in order for each `cycle()` call.
///   - After each simulated day, entity timestamps are shifted back by 86400 seconds
///     to simulate the passage of time.
///   - The CognitiveLoopService is constructed once and reused; its attention memory
///     persists across days as it would in production.
pub struct SimulationEngine {
    store: Arc<InMemoryWorldModelStore>,
    loop_svc: CognitiveLoopService,
    trace: Vec<DayTrace>,
}

impl SimulationEngine {
    /// Create a new simulation engine pre-populated with initial entities.
    ///
    /// `day_responses` should contain one StructuredWorldUpdate per cycle()-call.
    /// Days with observations consume one response; silent days consume none.
    pub async fn new(
        initial_entities: Vec<Entity>,
        day_responses: Vec<StructuredWorldUpdate>,
    ) -> Self {
        let store = Arc::new(InMemoryWorldModelStore::new());

        // Pre-populate world model
        for entity in initial_entities {
            store.insert_entity(entity).await;
        }

        // Coordinator for understanding (returns StructuredWorldUpdate JSON)
        let understanding_responses = Arc::new(Mutex::new(VecDeque::from(day_responses)));
        let understanding_coordinator = Arc::new(ScriptedCoordinator {
            responses: understanding_responses.clone(),
            counter: Arc::new(AtomicUsize::new(0)),
        });
        let understanding = WorldUnderstandingService::new(understanding_coordinator);

        // Coordinator for planner pipeline (returns default "respond" actions and memory evaluations)
        let planner_coordinator = Arc::new(DefaultPlannerCoordinator);

        let loop_svc = CognitiveLoopService::new(store.clone(), understanding, planner_coordinator);

        Self {
            store,
            loop_svc,
            trace: Vec::new(),
        }
    }

    /// Run one day of the simulation.
    ///
    /// If `observation` is Some, a full `cycle()` is executed.
    /// Then `tick()` is always called (self-initiated companion reflection).
    /// After both, entity timestamps are shifted by 86400s to simulate time passing.
    pub async fn run_day(&mut self, day: u64, day_data: &ScenarioDay) {
        let cycle_decision = if let Some(obs) = &day_data.observation {
            Some(self.loop_svc.cycle(obs).await)
        } else {
            None
        };

        let tick_decision = self.loop_svc.tick().await;

        // Build context from World Model (same logic as build_context())
        let context_entities = self.build_context().await;
        let context_entity_names: Vec<String> =
            context_entities.iter().map(|e| e.name.clone()).collect();
        let context_summary = Self::format_context_summary(&context_entities);

        // Reflections are logged internally (not in WM).
        // They are always recorded for every tick() call.
        let reflection_recorded = true;

        self.trace.push(DayTrace {
            day,
            observation: day_data.observation.clone(),
            cycle_decision,
            tick_decision,
            context_entity_names,
            context_summary,
            reflection_recorded,
        });

        // Shift all entity timestamps by 86400s to simulate time passage
        self.shift_timestamps().await;
    }

    /// Run a complete scenario (all days).
    pub async fn run_scenario(&mut self, days: &[ScenarioDay]) {
        for (i, day_data) in days.iter().enumerate() {
            let day_num = (i + 1) as u64;
            self.run_day(day_num, day_data).await;
        }
    }

    /// Re-implementation of CognitiveLoopService::build_context().
    async fn build_context(&self) -> Vec<Entity> {
        let entities = self.store.all_entities().await;
        let now = Timestamp::now();
        let mut scored: Vec<(f64, Entity)> = entities
            .into_iter()
            .filter(|e| matches!(e.lifecycle, EntityLifecycle::Active))
            .map(|e| {
                let age_secs =
                    (now.as_nanos() - e.updated_at.as_nanos()).abs() as f64 / 1_000_000_000.0;
                let recency = (1.0 - (age_secs / 86_400.0).min(1.0)).max(0.0);
                let score = e.importance as f64 * 0.5 + recency * 0.3 + e.confidence as f64 * 0.2;
                (score, e)
            })
            .collect();
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        scored.into_iter().take(5).map(|(_, e)| e).collect()
    }

    /// Re-implementation of CognitiveLoopService::format_context_summary().
    fn format_context_summary(entities: &[Entity]) -> String {
        if entities.is_empty() {
            return "No prior context — beginning fresh.".into();
        }
        let parts: Vec<String> = entities
            .iter()
            .map(|e| format!("{} ({})", e.name, e.entity_type))
            .collect();
        format!("Recently: {}", parts.join(", "))
    }

    /// Shift all entity timestamps back by 1 day.
    async fn shift_timestamps(&self) {
        let entities = self.store.all_entities().await;
        for mut entity in entities {
            let old_nanos = entity.updated_at.as_nanos();
            let new_nanos = old_nanos.saturating_sub(NANOS_PER_DAY);
            entity.updated_at = Timestamp::from_nanos(new_nanos);
            self.store.update_entity(&entity).await;
        }
    }

    /// Access the simulation trace.
    pub fn trace(&self) -> &[DayTrace] {
        &self.trace
    }

    /// Access the world model store (for post-simulation inspection).
    pub fn store(&self) -> &Arc<InMemoryWorldModelStore> {
        &self.store
    }

    /// Total cycles executed.
    pub fn total_cycles(&self) -> u64 {
        self.loop_svc.cycle_count()
    }
}

/// A coordinator that returns pre-defined responses in order.
#[derive(Debug)]
struct ScriptedCoordinator {
    responses: Arc<Mutex<VecDeque<StructuredWorldUpdate>>>,
    counter: Arc<AtomicUsize>,
}

#[async_trait::async_trait]
impl IntelligenceCoordinator for ScriptedCoordinator {
    async fn request(
        &self,
        _request: ModelRequest,
    ) -> Result<ModelResponse, intelligence_core::ModelError> {
        let response = {
            let mut queue = self.responses.lock().unwrap();
            queue.pop_front().unwrap_or_else(|| StructuredWorldUpdate {
                entities: vec![],
                relationships: vec![],
                state_changes: vec![],
                new_observations: vec![],
                open_questions: vec![],
                possible_hypotheses: vec![],
            })
        };
        let json = serde_json::to_string(&response).unwrap();
        Ok(ModelResponse {
            request_id: RequestId::new(),
            model_id: ModelId::new(),
            content: json,
            usage: TokenUsage {
                prompt_tokens: 0,
                completion_tokens: 0,
                total_tokens: 0,
            },
            finished: true,
            finish_reason: Some("stop".into()),
        })
    }

    async fn request_stream(
        &self,
        _request: ModelRequest,
    ) -> Result<Box<dyn Stream<Item = StreamChunk> + Send>, intelligence_core::ModelError> {
        unimplemented!("stream not used in simulation")
    }

    async fn embed(
        &self,
        _texts: &[String],
    ) -> Result<Vec<Embedding>, intelligence_core::ModelError> {
        unimplemented!("embed not used in simulation")
    }

    async fn health(&self) -> Result<(), intelligence_core::ModelError> {
        Ok(())
    }

    async fn pipeline_stats(&self) -> Result<IntelligenceStats, intelligence_core::ModelError> {
        unimplemented!("stats not used in simulation")
    }

    async fn conversation(
        &self,
        _id: &ConversationId,
    ) -> Result<Vec<ModelResponse>, intelligence_core::ModelError> {
        unimplemented!("conversation not used in simulation")
    }
}

/// A coordinator for the planner pipeline that returns default responses.
/// Returns "respond" as the plan action and default memory evaluations.
#[derive(Debug)]
struct DefaultPlannerCoordinator;

#[async_trait::async_trait]
impl IntelligenceCoordinator for DefaultPlannerCoordinator {
    async fn request(
        &self,
        request: ModelRequest,
    ) -> Result<ModelResponse, intelligence_core::ModelError> {
        let input_text = match &request.input {
            ModelInput::Text(t) => t.as_str(),
            _ => "",
        };
        let content = if input_text.contains("planning engine") {
            // Planner prompt: return plan JSON
            r#"{"actions":[{"type":"respond","reason":"default simulation response"}]}"#
        } else if input_text.contains("memory evaluator") {
            // Memory evaluator prompt: return storage decisions
            r#"[{"store":false,"importance":0.0,"confidence":0.0,"reason":"default simulation","summary":""}]"#
        } else {
            // Respond prompt: return a natural response
            "Hello! Everything is going well here."
        };
        Ok(ModelResponse {
            request_id: RequestId::new(),
            model_id: ModelId::new(),
            content: content.into(),
            usage: TokenUsage {
                prompt_tokens: 0,
                completion_tokens: 0,
                total_tokens: 0,
            },
            finished: true,
            finish_reason: Some("stop".into()),
        })
    }

    async fn request_stream(
        &self,
        _request: ModelRequest,
    ) -> Result<Box<dyn Stream<Item = StreamChunk> + Send>, intelligence_core::ModelError> {
        unimplemented!("stream not used in simulation")
    }

    async fn embed(
        &self,
        _texts: &[String],
    ) -> Result<Vec<Embedding>, intelligence_core::ModelError> {
        unimplemented!("embed not used in simulation")
    }

    async fn health(&self) -> Result<(), intelligence_core::ModelError> {
        Ok(())
    }

    async fn pipeline_stats(&self) -> Result<IntelligenceStats, intelligence_core::ModelError> {
        unimplemented!("stats not used in simulation")
    }

    async fn conversation(
        &self,
        _id: &ConversationId,
    ) -> Result<Vec<ModelResponse>, intelligence_core::ModelError> {
        unimplemented!("conversation not used in simulation")
    }
}
