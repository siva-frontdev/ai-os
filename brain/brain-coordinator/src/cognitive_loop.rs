use std::collections::VecDeque;
use std::sync::Arc;

use crate::attention::{AttentionDecision, AttentionEvaluator, AttentionOutcome};
use crate::evolution::EvolutionEngine;
use crate::planner::Planner;
use crate::planner::executor::ActionExecutor;
use crate::planner::memory_evaluator::MemoryEvaluator;
use crate::planner::runtime_executor::{RuntimeAwareExecutor, format_runtime_results};
use brain_core::types::Decision;
use intelligence_coordinator::world_understanding::WorldUnderstandingService;
use intelligence_core::traits::IntelligenceCoordinator;
use intelligence_core::types::{ModelRequest, RequestId};
use memory_core::Timestamp;
use memory_core::wm::{Entity, EntityLifecycle};
use memory_storage::wm_store::WorldModelStore;

/// A recorded reflection cycle stored outside the World Model.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ReflectionLogEntry {
    pub cycle: u64,
    pub outcome: &'static str,
    pub summary: String,
    pub timestamp: Timestamp,
}

/// A single turn in recent conversation history.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ConversationTurn {
    pub message: String,
    pub timestamp: Timestamp,
    pub is_from_user: bool,
}

/// The heart of the Continuous Cognitive Loop (ADR-0007).
///
/// Pipeline (after this refactor):
///   User Input
///       ↓
///   World Understanding   (interpret — AI produces structured understanding)
///       ↓
///   Memory Evaluator     (decide what knowledge to store, based on meaning)
///       ↓
///   Evolution Engine     (store selected knowledge in World Model)
///       ↓
///   Planner              (decide what actions to take, based on capabilities)
///       ↓
///   Action Executor      (execute the plan — Rust runs each action)
///       ↓
///   Decision             (Communicate, Wait, UpdateMemory, Execute)
///
/// Semantic decisions (what to store, what to retrieve, how to respond)
/// are made by the LLM via the Planner and Memory Evaluator.
/// Rust provides the execution machinery. No keyword rules.
#[derive(Debug)]
pub struct CognitiveLoopService {
    store: Arc<dyn WorldModelStore>,
    evolution: EvolutionEngine,
    understanding: WorldUnderstandingService,
    coordinator: Arc<dyn IntelligenceCoordinator>,
    attention: AttentionEvaluator,
    planner: Planner,
    memory_evaluator: MemoryEvaluator,
    executor: ActionExecutor,
    runtime: Option<Arc<RuntimeAwareExecutor>>,
    cycle_count: u64,
    reflection_log: VecDeque<ReflectionLogEntry>,
    last_communicated_at: Option<Timestamp>,
    consecutive_silent_ticks: u32,
    recent_messages: VecDeque<ConversationTurn>,
}

impl CognitiveLoopService {
    pub fn new(
        world_model: Arc<dyn WorldModelStore>,
        understanding: WorldUnderstandingService,
        coordinator: Arc<dyn IntelligenceCoordinator>,
    ) -> Self {
        Self {
            store: world_model.clone(),
            evolution: EvolutionEngine::new(world_model.clone()),
            understanding,
            coordinator: coordinator.clone(),
            attention: AttentionEvaluator::new(),
            planner: Planner::new(coordinator.clone()),
            memory_evaluator: MemoryEvaluator::new(coordinator.clone()),
            executor: ActionExecutor::new(world_model, coordinator),
            runtime: None,
            cycle_count: 0,
            reflection_log: VecDeque::new(),
            last_communicated_at: None,
            consecutive_silent_ticks: 0,
            recent_messages: VecDeque::new(),
        }
    }

    /// Attach a runtime-aware executor so plan actions that target runtime
    /// capabilities (`email.send`, `telegram.inject_inbound`, ...) are
    /// dispatched through the real runtime, and their results ground the
    /// final response.
    ///
    /// Also registers the runtime capabilities into the planner so it can
    /// propose them. Call before [`CognitiveLoopService::cycle`].
    pub async fn set_runtime(&mut self, runtime: Arc<RuntimeAwareExecutor>) {
        self.runtime = Some(runtime);
    }

    /// Register a single runtime capability into the planner's registry so
    /// the Planner can propose actions for it.
    pub fn register_runtime_capability(
        &mut self,
        name: impl Into<String>,
        description: impl Into<String>,
    ) {
        self.planner
            .capabilities_mut()
            .register(crate::planner::Capability {
                name: name.into(),
                description: description.into(),
            });
    }

    /// Run one full cognitive cycle on the given text observation.
    ///
    /// Pipeline: Understanding → Memory Evaluation → Evolution → Planner → Execution
    pub async fn cycle(&mut self, observation: &str) -> Decision {
        self.cycle_count += 1;

        // Record user message in conversation history
        self.recent_messages.push_back(ConversationTurn {
            message: observation.to_string(),
            timestamp: Timestamp::now(),
            is_from_user: true,
        });
        if self.recent_messages.len() > 10 {
            self.recent_messages.pop_front();
        }

        // Structural duplicate detection (same message text, not semantic)
        let is_duplicate = self.detect_duplicate(observation);
        if let Some(ref original) = is_duplicate {
            tracing::info!("duplicate message detected (original: {original})");
        }

        // Temporal session gap check (time elapsed, not semantic)
        let is_new_session = self.has_session_gap();

        // 1. Build continuity context from World Model
        let context = self.build_context().await;
        let context_summary = Self::format_context_summary(&context, &self.recent_messages);

        // 2. World Understanding — AI-driven structured interpretation
        let understanding = match self.understanding.understand(observation).await {
            Ok(u) => u,
            Err(e) => {
                tracing::warn!("World understanding failed: {e}");
                return Decision::Wait;
            }
        };

        // 3. Memory Evaluator — decide what knowledge to store (meaning-based)
        let stored_update = self
            .memory_evaluator
            .evaluate(observation, &understanding)
            .await;

        // 4. Evolution Engine — store selected knowledge in World Model
        let report = match stored_update {
            Some(ref update) => {
                tracing::debug!(
                    "storing {} entities from understanding",
                    update.entities.len()
                );
                self.evolution.evolve(update).await
            }
            None => {
                tracing::debug!("memory evaluator declined to store anything");
                crate::evolution::EvolutionReport::default()
            }
        };

        // 5. Evaluate Attention — does this deserve reflection?
        let attention = self.attention.evaluate(&understanding, &report);

        // 6. Planner — decide what actions to take
        let has_prior_knowledge = !context.is_empty();
        let recent_text: Vec<String> = self
            .recent_messages
            .iter()
            .rev()
            .take(6)
            .map(|t| {
                let who = if t.is_from_user { "User" } else { "You" };
                format!("{who}: {}", t.message)
            })
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();

        let plan = self
            .planner
            .plan(
                observation,
                &recent_text.join("\n"),
                &context_summary,
                has_prior_knowledge,
            )
            .await;

        tracing::debug!("Planner produced {} actions", plan.actions.len());

        // 6b. Dispatch runtime-capability actions through the runtime (if any).
        //     In-memory actions are skipped here; the executor handles those.
        let runtime_context = match &self.runtime {
            Some(executor) => format_runtime_results(&executor.execute_plan(&plan).await),
            None => String::new(),
        };

        // 7. Action Executor — execute the plan
        let mut decision = self
            .executor
            .execute(
                &plan,
                observation,
                &self.recent_messages,
                &context_summary,
                &context,
                has_prior_knowledge,
                is_duplicate,
                is_new_session,
                &runtime_context,
            )
            .await;

        // Honor AskUser from attention — AI genuinely needs input
        if let AttentionOutcome::AskUser { ref question } = attention.outcome {
            decision = Decision::Communicate {
                recipient: "user".into(),
                message: question.clone(),
                reason: "AI needs user input to proceed".into(),
            };
        }

        // Record companion response in conversation history
        if let Decision::Communicate { ref message, .. } = decision {
            self.recent_messages.push_back(ConversationTurn {
                message: message.clone(),
                timestamp: Timestamp::now(),
                is_from_user: false,
            });
            if self.recent_messages.len() > 10 {
                self.recent_messages.pop_front();
            }
        }

        decision
    }

    /// Number of cycles executed since creation.
    pub fn cycle_count(&self) -> u64 {
        self.cycle_count
    }

    /// Build continuity context directly from the World Model.
    ///
    /// Scores all active entities by importance × recency × confidence
    /// and returns the top K. No domain-specific summary types,
    /// no parallel caches, just generic graph queries.
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

    /// Format a natural-language summary from context entities.
    /// Never exposes entity names, types, or counts.
    fn format_context_summary(entities: &[Entity], _recent: &VecDeque<ConversationTurn>) -> String {
        if entities.is_empty() {
            return String::new();
        }
        let has_project = entities
            .iter()
            .any(|e| e.entity_type == "project" || e.entity_type == "task");
        let has_skill = entities.iter().any(|e| e.entity_type == "skill");
        let has_plan = entities.iter().any(|e| e.entity_type == "plan");

        let mut topics = Vec::new();
        if has_project {
            let names: Vec<&str> = entities
                .iter()
                .filter(|e| e.entity_type == "project" || e.entity_type == "task")
                .map(|e| e.name.as_str())
                .collect();
            if names.len() == 1 {
                topics.push(format!("your work on {}", names[0]));
            } else {
                let last = names.last().unwrap();
                let rest = &names[..names.len() - 1];
                topics.push(format!("your work on {} and {}", rest.join(", "), last));
            }
        }
        if has_skill {
            let names: Vec<&str> = entities
                .iter()
                .filter(|e| e.entity_type == "skill")
                .map(|e| e.name.as_str())
                .collect();
            if names.len() == 1 {
                topics.push(format!("your interest in {}", names[0]));
            } else {
                let last = names.last().unwrap();
                let rest = &names[..names.len() - 1];
                topics.push(format!("your interest in {} and {}", rest.join(", "), last));
            }
        }
        if has_plan {
            topics.push("a plan you're working on".to_string());
        }

        if topics.is_empty() {
            return String::new();
        }

        let joined = topics.join(", ");
        format!("I remember {}", joined)
    }

    /// Run a self-initiated reflection cycle (no user input).
    ///
    /// The companion periodically calls this to ask:
    /// "Given everything I know, what would genuinely help right now?"
    ///
    /// No new observation is processed. Attention is evaluated on the
    /// World Model state alone (stagnation, importance, recency).
    /// The AI only communicates when there is a positive justification:
    /// stagnation of an important entity, meaningful progress, or a
    /// significant change. Context alone is not sufficient.
    pub async fn tick(&mut self) -> Decision {
        self.cycle_count += 1;
        let context = self.build_context().await;
        let context_summary = Self::format_context_summary(&context, &self.recent_messages);
        let attention = self.attention.evaluate_context(&context);

        let decision = match attention.outcome {
            AttentionOutcome::Ignore
            | AttentionOutcome::ObserveLater { .. }
            | AttentionOutcome::ReflectSoon { .. } => Decision::Wait,

            AttentionOutcome::ReflectNow => {
                self.decide_on_context(&context, &context_summary, &attention)
            }

            AttentionOutcome::AskUser { question } => Decision::Communicate {
                recipient: "user".into(),
                message: question,
                reason: "companion reflection".into(),
            },
            AttentionOutcome::Suggest { message } | AttentionOutcome::Notify { message } => {
                Decision::Communicate {
                    recipient: "user".into(),
                    message,
                    reason: "companion reflection".into(),
                }
            }
            AttentionOutcome::Escalate { reason } => Decision::Communicate {
                recipient: "user".into(),
                message: reason,
                reason: "companion reflection".into(),
            },
        };

        self.record_reflection(&decision, &context_summary).await;
        if matches!(decision, Decision::Wait) {
            self.consecutive_silent_ticks += 1;
        } else {
            self.consecutive_silent_ticks = 0;
            self.last_communicated_at = Some(Timestamp::now());
        }

        decision
    }

    /// Decide what to communicate during a tick (self-initiated reflection).
    ///
    /// Requires positive justification before communicating.
    fn decide_on_context(
        &self,
        context_entities: &[Entity],
        _context_summary: &str,
        attention: &AttentionDecision,
    ) -> Decision {
        if context_entities.is_empty() {
            return Decision::Wait;
        }

        let stagnation_strength = attention
            .signals
            .iter()
            .find(|s| s.name == "stagnation")
            .map(|s| s.strength)
            .unwrap_or(0.0);

        let is_first_ever = self.last_communicated_at.is_none();
        if !is_first_ever && self.consecutive_silent_ticks < 2 {
            return Decision::Wait;
        }

        if stagnation_strength > 0.15 {
            let entity = context_entities
                .iter()
                .max_by(|a, b| a.importance.partial_cmp(&b.importance).unwrap());
            if let Some(entity) = entity {
                let message = Self::stagnation_message(entity);
                return Decision::Communicate {
                    recipient: "user".into(),
                    message,
                    reason: "checking in on prior conversation".into(),
                };
            }
        }

        let max_importance = context_entities
            .iter()
            .map(|e| e.importance as f64)
            .fold(0.0f64, f64::max);
        if max_importance > 0.8 && self.consecutive_silent_ticks >= 3 {
            let entity = context_entities
                .iter()
                .max_by(|a, b| a.importance.partial_cmp(&b.importance).unwrap())
                .unwrap();
            let message = Self::stagnation_message(entity);
            return Decision::Communicate {
                recipient: "user".into(),
                message,
                reason: "follow-up after quiet period".into(),
            };
        }

        Decision::Wait
    }

    /// Build a follow-up message for a stagnant entity.
    fn stagnation_message(entity: &Entity) -> String {
        match entity.entity_type.as_str() {
            "project" | "task" => {
                let starters = [
                    "I noticed we haven't talked about {name} in a while",
                    "Just thinking about {name} — how's it going?",
                    "How is {name} coming along?",
                    "I was wondering about the status of {name}",
                ];
                let starter = starters[entity.name.len() as usize % starters.len()];
                format!("{}?", starter.replace("{name}", &entity.name))
            }
            "person" | "user" => {
                let starters = [
                    "I haven't heard from {name} lately",
                    "How are things going, {name}?",
                    "I was thinking about {name} today",
                ];
                let starter = starters[entity.name.len() as usize % starters.len()];
                format!("{}?", starter.replace("{name}", &entity.name))
            }
            "skill" => {
                let starters = [
                    "How is your progress on {name} going?",
                    "Have you had a chance to work on {name} recently?",
                    "I was thinking about {name} — making any progress?",
                ];
                let starter = starters[entity.name.len() as usize % starters.len()];
                format!("{}?", starter.replace("{name}", &entity.name))
            }
            _ => {
                format!("I noticed {name} — anything new?", name = entity.name,)
            }
        }
    }

    fn current_hour() -> u32 {
        let now = Timestamp::now();
        let secs = now.as_secs();
        ((secs / 3600) % 24) as u32
    }

    /// Record a reflection decision in the internal log.
    async fn record_reflection(&mut self, decision: &Decision, summary: &str) {
        let outcome = match decision {
            Decision::Wait => "silent",
            Decision::Communicate { .. } => "communicated",
            Decision::UpdateMemory { .. } => "updated_memory",
            Decision::Execute { .. } => "executed",
        };

        self.reflection_log.push_back(ReflectionLogEntry {
            cycle: self.cycle_count,
            outcome,
            summary: summary.into(),
            timestamp: Timestamp::now(),
        });

        tracing::info!(
            cycle = self.cycle_count,
            outcome = outcome,
            summary = summary,
            "reflection recorded"
        );
    }

    /// Access the reflection log.
    pub fn reflection_log(&self) -> &VecDeque<ReflectionLogEntry> {
        &self.reflection_log
    }

    // ── Structural (non-semantic) helpers ─────────────────────

    /// Detect if the same user message text was sent within a recent window.
    /// This is a structural/textual check, not a semantic rule.
    fn detect_duplicate(&self, message: &str) -> Option<String> {
        let normalized = message.trim().to_lowercase();
        let user_messages: Vec<&str> = self
            .recent_messages
            .iter()
            .filter(|t| t.is_from_user)
            .map(|t| t.message.as_str())
            .collect();
        for prev in user_messages.iter().rev().take(5) {
            let prev_norm = prev.trim().to_lowercase();
            if normalized == prev_norm {
                return Some((*prev).to_string());
            }
            if normalized.len() > 8 && prev_norm.len() > 8 {
                if normalized.contains(&prev_norm) || prev_norm.contains(&normalized) {
                    return Some((*prev).to_string());
                }
            }
        }
        None
    }

    /// Check whether this is the user's first message in a new session.
    /// This is a temporal check (time elapsed), not a semantic rule.
    fn has_session_gap(&self) -> bool {
        let user_turns: Vec<&ConversationTurn> = self
            .recent_messages
            .iter()
            .filter(|t| t.is_from_user)
            .collect();
        if user_turns.is_empty() && self.cycle_count > 0 {
            return true;
        }
        if let Some(last) = user_turns.last() {
            let elapsed = Timestamp::now().as_secs() - last.timestamp.as_secs();
            if elapsed > 3600 {
                return true;
            }
        }
        false
    }

    /// Template fallback when the Planner/Executor pipeline fails.
    /// Uses simple rules, not keywords.
    fn decide(
        &self,
        understanding: &intelligence_coordinator::world_understanding::StructuredWorldUpdate,
        context_entities: &[Entity],
        context_summary: &str,
    ) -> Decision {
        let has_continuity = !context_entities.is_empty();
        let greeting = Self::time_of_day_greeting();

        if let Some(entity) = understanding.entities.first() {
            let is_person = matches!(entity.entity_type.as_str(), "person" | "user");
            let name = if entity.name.to_lowercase() == "user" {
                "".to_string()
            } else {
                entity.name.clone()
            };

            let message = if is_person {
                if has_continuity {
                    if context_summary.is_empty() {
                        format!("{}! Good to see you again.", greeting)
                    } else {
                        format!("{}! Good to see you. {}", greeting, context_summary)
                    }
                } else {
                    format!(
                        "{}. I don't know much about you yet, but I'm here to learn. What's on your mind?",
                        greeting
                    )
                }
            } else {
                if has_continuity {
                    if context_summary.is_empty() {
                        format!("{}! That's interesting — tell me more.", greeting)
                    } else {
                        format!("{}! {}. How is it going?", greeting, context_summary)
                    }
                } else {
                    format!(
                        "{}! I see you're working on something. What's the latest?",
                        greeting
                    )
                }
            };
            return Decision::Communicate {
                recipient: if name.is_empty() { "user".into() } else { name },
                message,
                reason: format!("responded to: {}", entity.name),
            };
        }

        if !understanding.new_observations.is_empty() {
            return Decision::Communicate {
                recipient: "user".into(),
                message: "I'll keep that in mind.".into(),
                reason: "new observation".into(),
            };
        }

        if !understanding.relationships.is_empty() {
            let rel = &understanding.relationships[0];
            return Decision::UpdateMemory {
                entity_name: rel.target.clone(),
                entity_type: "entity".into(),
                properties: vec![("related".into(), rel.relationship_type.clone())],
                reason: format!("relation: {} -> {}", rel.source, rel.target),
            };
        }

        if !understanding.state_changes.is_empty() {
            let sc = &understanding.state_changes[0];
            return Decision::UpdateMemory {
                entity_name: sc.entity_name.clone(),
                entity_type: "state".into(),
                properties: vec![(sc.attribute.clone(), sc.new_value.clone())],
                reason: format!("state: {} = {}", sc.entity_name, sc.new_value),
            };
        }

        Decision::Wait
    }

    /// Returns a natural time-of-day greeting.
    fn time_of_day_greeting() -> &'static str {
        let hour = Self::current_hour();
        match hour {
            0..=4 => "Hey",
            5..=11 => "Good morning",
            12..=16 => "Good afternoon",
            17..=21 => "Good evening",
            _ => "Hey",
        }
    }
}

// ── Reusable Mock Infrastructure ───────────────────────────

/// Mock coordinator that returns a predetermined StructuredWorldUpdate.
/// Used across crate boundaries for testing planner, executor, etc.
#[derive(Debug)]
pub struct MockCoordinator(
    pub intelligence_coordinator::world_understanding::StructuredWorldUpdate,
);

#[async_trait::async_trait]
impl IntelligenceCoordinator for MockCoordinator {
    async fn request(
        &self,
        _request: ModelRequest,
    ) -> Result<intelligence_core::types::ModelResponse, intelligence_core::ModelError> {
        let json = serde_json::to_string(&self.0).unwrap();
        Ok(intelligence_core::types::ModelResponse {
            request_id: intelligence_core::types::RequestId::new(),
            model_id: intelligence_core::types::ModelId::new(),
            content: json,
            usage: intelligence_core::types::TokenUsage {
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
    ) -> Result<
        Box<dyn futures::Stream<Item = intelligence_core::types::StreamChunk> + Send>,
        intelligence_core::ModelError,
    > {
        unimplemented!("stream not used in tests")
    }

    async fn embed(
        &self,
        _texts: &[String],
    ) -> Result<Vec<intelligence_core::types::Embedding>, intelligence_core::ModelError> {
        unimplemented!("embed not used in tests")
    }

    async fn health(&self) -> Result<(), intelligence_core::ModelError> {
        Ok(())
    }

    async fn pipeline_stats(
        &self,
    ) -> Result<intelligence_core::types::IntelligenceStats, intelligence_core::ModelError> {
        unimplemented!("stats not used in tests")
    }

    async fn conversation(
        &self,
        _id: &intelligence_core::types::ConversationId,
    ) -> Result<Vec<intelligence_core::types::ModelResponse>, intelligence_core::ModelError> {
        unimplemented!("conversation not used in tests")
    }
}

/// Empty understanding for test convenience.
pub fn empty_understanding() -> intelligence_coordinator::world_understanding::StructuredWorldUpdate
{
    intelligence_coordinator::world_understanding::StructuredWorldUpdate {
        entities: vec![],
        relationships: vec![],
        state_changes: vec![],
        new_observations: vec![],
        open_questions: vec![],
        possible_hypotheses: vec![],
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use super::CognitiveLoopService;
    use brain_core::types::Decision;
    use intelligence_coordinator::world_understanding::{
        StructuredWorldUpdate, WorldEntity, WorldRelationship, WorldUnderstandingService,
    };
    use intelligence_core::traits::IntelligenceCoordinator;
    use memory_core::Timestamp;
    use memory_core::wm::Entity;
    use memory_storage::wm_store::InMemoryWorldModelStore;
    use memory_storage::wm_store::WorldModelStore;

    /// Test coordinator that returns predetermined JSON for planner,
    /// memory evaluator, and response LLM calls. Each call type is
    /// identified by keywords in the prompt text.
    #[derive(Debug)]
    struct TestCoordinator {
        plan_json: String,
        memory_json: String,
        response_text: String,
    }

    impl TestCoordinator {
        fn new(
            plan_json: &str,
            memory_json: &str,
            response_text: &str,
        ) -> Arc<dyn IntelligenceCoordinator> {
            Arc::new(Self {
                plan_json: plan_json.to_string(),
                memory_json: memory_json.to_string(),
                response_text: response_text.to_string(),
            })
        }

        fn default_respond() -> Arc<dyn IntelligenceCoordinator> {
            Self::new(
                r#"{"actions":[{"type":"respond","reason":"test"}]}"#,
                r#"[{"store":true,"importance":0.5,"confidence":0.5,"reason":"test","summary":""}]"#,
                "Hello! How can I help you today?",
            )
        }
    }

    #[async_trait::async_trait]
    impl intelligence_core::traits::IntelligenceCoordinator for TestCoordinator {
        async fn request(
            &self,
            request: intelligence_core::types::ModelRequest,
        ) -> Result<intelligence_core::types::ModelResponse, intelligence_core::ModelError>
        {
            let input = match &request.input {
                intelligence_core::types::ModelInput::Text(t) => t.as_str(),
                _ => "",
            };
            let content =
                if input.contains("planning engine") || input.contains("plan what actions") {
                    self.plan_json.clone()
                } else if input.contains("memory evaluator") {
                    self.memory_json.clone()
                } else {
                    self.response_text.clone()
                };
            Ok(intelligence_core::types::ModelResponse {
                request_id: intelligence_core::types::RequestId::new(),
                model_id: intelligence_core::types::ModelId::new(),
                content,
                usage: intelligence_core::types::TokenUsage {
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
            _request: intelligence_core::types::ModelRequest,
        ) -> Result<
            Box<dyn futures::Stream<Item = intelligence_core::types::StreamChunk> + Send>,
            intelligence_core::ModelError,
        > {
            unimplemented!("stream not used in tests")
        }

        async fn embed(
            &self,
            _texts: &[String],
        ) -> Result<Vec<intelligence_core::types::Embedding>, intelligence_core::ModelError>
        {
            unimplemented!("embed not used in tests")
        }

        async fn health(&self) -> Result<(), intelligence_core::ModelError> {
            Ok(())
        }

        async fn pipeline_stats(
            &self,
        ) -> Result<intelligence_core::types::IntelligenceStats, intelligence_core::ModelError>
        {
            unimplemented!("stats not used in tests")
        }

        async fn conversation(
            &self,
            _id: &intelligence_core::types::ConversationId,
        ) -> Result<Vec<intelligence_core::types::ModelResponse>, intelligence_core::ModelError>
        {
            unimplemented!("conversation not used in tests")
        }
    }

    /// Create a CognitiveLoopService with mock understanding and a test coordinator.
    /// `ignore` controls whether the planner returns "ignore" (true) or "respond" (false).
    fn make_loop(
        store: Arc<dyn WorldModelStore>,
        understanding: WorldUnderstandingService,
    ) -> CognitiveLoopService {
        let coordinator = TestCoordinator::new(
            r#"{"actions":[{"type":"respond","reason":"test"}]}"#,
            r#"[{"store":true,"importance":0.5,"confidence":0.5,"reason":"test","summary":""}]"#,
            "Hello! How can I help you today?",
        );
        CognitiveLoopService::new(store, understanding, coordinator)
    }

    /// Create a loop where the planner returns "ignore" (producing Wait).
    fn make_ignore_loop(
        store: Arc<dyn WorldModelStore>,
        understanding: WorldUnderstandingService,
    ) -> CognitiveLoopService {
        let coordinator = TestCoordinator::new(
            r#"{"actions":[{"type":"ignore","reason":"test"}]}"#,
            r#"[{"store":false,"importance":0.0,"confidence":0.0,"reason":"test","summary":""}]"#,
            "",
        );
        CognitiveLoopService::new(store, understanding, coordinator)
    }

    /// Mock understanding that returns a predetermined StructuredWorldUpdate.
    fn mock_understanding(result: StructuredWorldUpdate) -> WorldUnderstandingService {
        WorldUnderstandingService::new(Arc::new(super::MockCoordinator(result)))
    }

    #[tokio::test]
    async fn test_wait_on_empty_understanding() {
        let store = Arc::new(InMemoryWorldModelStore::new());
        let result = StructuredWorldUpdate {
            entities: vec![],
            relationships: vec![],
            state_changes: vec![],
            new_observations: vec![],
            open_questions: vec![],
            possible_hypotheses: vec![],
        };
        let empty = mock_understanding(result);
        let mut loop_svc = make_ignore_loop(store, empty);
        let decision = loop_svc.cycle("nothing here").await;
        assert!(matches!(decision, Decision::Wait));
        assert_eq!(loop_svc.cycle_count(), 1);
    }

    #[tokio::test]
    async fn test_person_triggers_communicate() {
        let store = Arc::new(InMemoryWorldModelStore::new());
        let result = StructuredWorldUpdate {
            entities: vec![WorldEntity {
                name: "Alice".into(),
                entity_type: "person".into(),
                properties: HashMap::new(),
                confidence: 0.9,
                importance: 0.7,
            }],
            relationships: vec![],
            state_changes: vec![],
            new_observations: vec![],
            open_questions: vec![],
            possible_hypotheses: vec![],
        };
        let svc = mock_understanding(result);
        let mut loop_svc = make_loop(store, svc);
        let decision = loop_svc.cycle("I am Alice").await;
        assert!(
            matches!(decision, Decision::Communicate { .. }),
            "person should produce Communicate, got {decision:?}"
        );
    }

    #[tokio::test]
    async fn test_relationship_requires_sufficient_attention() {
        let store = Arc::new(InMemoryWorldModelStore::new());
        let result = StructuredWorldUpdate {
            entities: vec![WorldEntity {
                name: "MacBook".into(),
                entity_type: "device".into(),
                properties: HashMap::new(),
                confidence: 0.8,
                importance: 0.6,
            }],
            relationships: vec![WorldRelationship {
                source: "Alice".into(),
                target: "MacBook".into(),
                relationship_type: "owns".into(),
                confidence: 0.85,
                weight: 0.9,
            }],
            state_changes: vec![],
            new_observations: vec![],
            open_questions: vec![],
            possible_hypotheses: vec![],
        };
        let svc = mock_understanding(result);
        let mut loop_svc = make_ignore_loop(store, svc);
        let decision = loop_svc.cycle("I bought a MacBook").await;
        assert!(
            matches!(decision, Decision::Wait),
            "weak signals should produce Wait, got {decision:?}"
        );
    }

    #[tokio::test]
    async fn test_trivial_observation_waits() {
        let store = Arc::new(InMemoryWorldModelStore::new());
        let result = StructuredWorldUpdate {
            entities: vec![],
            relationships: vec![],
            state_changes: vec![],
            new_observations: vec!["User mentioned reading a book".into()],
            open_questions: vec![],
            possible_hypotheses: vec![],
        };
        let svc = mock_understanding(result);
        let mut loop_svc = make_ignore_loop(store, svc);
        let decision = loop_svc.cycle("reading a book").await;
        assert!(
            matches!(decision, Decision::Wait),
            "trivial observation should wait, got {decision:?}"
        );
    }

    #[tokio::test]
    async fn test_state_change_triggers_update_memory() {
        let store = Arc::new(InMemoryWorldModelStore::new());
        let result = StructuredWorldUpdate {
            entities: vec![WorldEntity {
                name: "Alice".into(),
                entity_type: "person".into(),
                properties: HashMap::new(),
                confidence: 0.9,
                importance: 0.7,
            }],
            relationships: vec![],
            state_changes: vec![intelligence_coordinator::world_understanding::StateChange {
                entity_name: "Alice".into(),
                attribute: "mood".into(),
                old_value: None,
                new_value: "tired".into(),
            }],
            new_observations: vec![],
            open_questions: vec![],
            possible_hypotheses: vec![],
        };
        let svc = mock_understanding(result);
        let mut loop_svc = make_loop(store.clone(), svc);
        let decision = loop_svc.cycle("I am tired").await;
        // With the new pipeline, state changes are stored by memory evaluator + evolution.
        // Verify entities were persisted in the World Model.
        let all = store.all_entities().await;
        let alice = all.iter().find(|e| e.name == "Alice");
        assert!(
            alice.is_some(),
            "Alice should have been stored in world model"
        );
        assert_eq!(alice.unwrap().entity_type, "person");
    }

    #[tokio::test]
    async fn test_cycle_stores_entities_in_world_model() {
        let store = Arc::new(InMemoryWorldModelStore::new());
        let result = StructuredWorldUpdate {
            entities: vec![
                WorldEntity {
                    name: "Alice".into(),
                    entity_type: "person".into(),
                    properties: HashMap::new(),
                    confidence: 0.9,
                    importance: 0.7,
                },
                WorldEntity {
                    name: "Rust".into(),
                    entity_type: "skill".into(),
                    properties: HashMap::new(),
                    confidence: 0.8,
                    importance: 0.6,
                },
            ],
            relationships: vec![WorldRelationship {
                source: "Alice".into(),
                target: "Rust".into(),
                relationship_type: "learning".into(),
                confidence: 0.85,
                weight: 0.7,
            }],
            state_changes: vec![],
            new_observations: vec![],
            open_questions: vec![],
            possible_hypotheses: vec![],
        };
        let svc = mock_understanding(result);
        let mut loop_svc = make_loop(store.clone(), svc);
        let _ = loop_svc.cycle("learning Rust").await;

        let entities = store.all_entities().await;
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"Alice"), "Alice should be in WM");
        assert!(names.contains(&"Rust"), "Rust should be in WM");
    }

    #[tokio::test]
    async fn test_understanding_error_returns_wait() {
        let store = Arc::new(InMemoryWorldModelStore::new());
        #[derive(Debug)]
        struct FailingCoordinator;

        #[async_trait::async_trait]
        impl intelligence_core::traits::IntelligenceCoordinator for FailingCoordinator {
            async fn request(
                &self,
                _request: intelligence_core::types::ModelRequest,
            ) -> Result<intelligence_core::types::ModelResponse, intelligence_core::ModelError>
            {
                Err(intelligence_core::ModelError::ServerError {
                    provider: "mock".into(),
                    status: 500,
                })
            }
            async fn request_stream(
                &self,
                _request: intelligence_core::types::ModelRequest,
            ) -> Result<
                Box<dyn futures::Stream<Item = intelligence_core::types::StreamChunk> + Send>,
                intelligence_core::ModelError,
            > {
                Err(intelligence_core::ModelError::ServerError {
                    provider: "mock".into(),
                    status: 500,
                })
            }
            async fn embed(
                &self,
                _texts: &[String],
            ) -> Result<Vec<intelligence_core::types::Embedding>, intelligence_core::ModelError>
            {
                Err(intelligence_core::ModelError::ServerError {
                    provider: "mock".into(),
                    status: 500,
                })
            }
            async fn health(&self) -> Result<(), intelligence_core::ModelError> {
                Ok(())
            }
            async fn pipeline_stats(
                &self,
            ) -> Result<intelligence_core::types::IntelligenceStats, intelligence_core::ModelError>
            {
                Err(intelligence_core::ModelError::ServerError {
                    provider: "mock".into(),
                    status: 500,
                })
            }
            async fn conversation(
                &self,
                _id: &intelligence_core::types::ConversationId,
            ) -> Result<Vec<intelligence_core::types::ModelResponse>, intelligence_core::ModelError>
            {
                Err(intelligence_core::ModelError::ServerError {
                    provider: "mock".into(),
                    status: 500,
                })
            }
        }

        let svc = WorldUnderstandingService::new(Arc::new(FailingCoordinator));
        let mut loop_svc = make_loop(store, svc);
        let decision = loop_svc.cycle("anything").await;
        assert!(matches!(decision, Decision::Wait));
    }

    // ── Daily Companion Tests ────────────────────────────────

    #[tokio::test]
    async fn test_companion_tick_empty_wm_stays_silent() {
        let store = Arc::new(InMemoryWorldModelStore::new());
        let svc = mock_understanding(super::empty_understanding());
        let mut loop_svc = make_loop(store, svc);
        let decision = loop_svc.tick().await;
        assert!(
            matches!(decision, Decision::Wait),
            "empty companion should stay silent, got {decision:?}"
        );
    }

    #[tokio::test]
    async fn test_companion_tick_stays_silent_with_fresh_entity() {
        let store = Arc::new(InMemoryWorldModelStore::new());
        let entity = Entity::new("project", "AI-OS")
            .with_importance(0.85)
            .with_confidence(0.9);
        store.insert_entity(entity).await;

        let svc = mock_understanding(super::empty_understanding());
        let mut loop_svc = make_loop(store, svc);
        let decision = loop_svc.tick().await;
        assert!(
            matches!(decision, Decision::Wait),
            "fresh entity without stagnation should stay silent, got {decision:?}"
        );
    }

    #[tokio::test]
    async fn test_companion_reflection_recorded_in_log() {
        let store = Arc::new(InMemoryWorldModelStore::new());
        let entity = Entity::new("project", "AI-OS")
            .with_importance(0.85)
            .with_confidence(0.9);
        store.insert_entity(entity).await;

        let svc = mock_understanding(super::empty_understanding());
        let mut loop_svc = make_loop(store.clone(), svc);
        let _ = loop_svc.tick().await;

        let log = loop_svc.reflection_log();
        assert!(!log.is_empty(), "tick should create a reflection log entry");
        assert_eq!(log[0].cycle, 1, "first cycle should be 1");
        assert_eq!(
            log[0].outcome, "silent",
            "empty WM should produce silent outcome"
        );
        assert!(!log[0].summary.is_empty(), "summary should not be empty");

        // Verify nothing was stored in WM
        let all = store.all_entities().await;
        let reflections: Vec<&str> = all
            .iter()
            .filter(|e| e.entity_type == "reflection")
            .map(|e| e.name.as_str())
            .collect();
        assert!(reflections.is_empty(), "reflections should NOT be in WM");
    }

    #[tokio::test]
    async fn test_companion_stagnation_triggers_follow_up() {
        let store = Arc::new(InMemoryWorldModelStore::new());
        let mut entity = Entity::new("project", "AI-OS")
            .with_importance(0.85)
            .with_confidence(0.9);
        let five_days_ago =
            Timestamp::from_nanos(Timestamp::now().as_nanos() - 5 * 86400 * 1_000_000_000);
        entity.updated_at = five_days_ago;
        store.insert_entity(entity).await;

        let svc = mock_understanding(super::empty_understanding());
        let mut loop_svc = make_loop(store, svc);
        let decision = loop_svc.tick().await;
        assert!(
            matches!(decision, Decision::Communicate { .. }),
            "stagnant high-importance project should trigger communication, got {decision:?}"
        );
    }

    #[tokio::test]
    async fn test_companion_daily_rhythm_natural_continuation() {
        let store = Arc::new(InMemoryWorldModelStore::new());
        let result = StructuredWorldUpdate {
            entities: vec![WorldEntity {
                name: "AI-OS".into(),
                entity_type: "project".into(),
                properties: HashMap::new(),
                confidence: 0.85,
                importance: 0.8,
            }],
            relationships: vec![],
            state_changes: vec![],
            new_observations: vec![],
            open_questions: vec![],
            possible_hypotheses: vec![],
        };
        let svc = mock_understanding(result);
        let mut loop_svc = make_loop(store.clone(), svc);

        // Day 1 morning: user starts working on AI-OS
        let morning = loop_svc.cycle("I'm building AI-OS").await;
        assert!(
            matches!(morning, Decision::Communicate { .. }),
            "first interaction should get a response, got {morning:?}"
        );

        // The cycle should have stored the entity via memory evaluator + evolution
        let entities = store.all_entities().await;
        let ai_os = entities.iter().find(|e| e.name == "AI-OS");
        assert!(ai_os.is_some(), "AI-OS should be in the World Model");
    }

    #[tokio::test]
    async fn test_companion_multiple_entities_stagnation_follow_up() {
        let store = Arc::new(InMemoryWorldModelStore::new());

        let mut project = Entity::new("project", "AI-OS")
            .with_importance(0.85)
            .with_confidence(0.9);
        let six_days_ago =
            Timestamp::from_nanos(Timestamp::now().as_nanos() - 6 * 86400 * 1_000_000_000);
        project.updated_at = six_days_ago;
        store.insert_entity(project).await;

        let mut skill = Entity::new("skill", "Rust")
            .with_importance(0.7)
            .with_confidence(0.8);
        skill.updated_at = six_days_ago;
        store.insert_entity(skill).await;

        let svc = mock_understanding(super::empty_understanding());
        let mut loop_svc = make_loop(store, svc);
        let decision = loop_svc.tick().await;

        assert!(
            matches!(decision, Decision::Communicate { .. }),
            "stagnation of important entities should trigger follow-up, got {decision:?}"
        );
        if let Decision::Communicate { message, .. } = &decision {
            assert!(
                message.contains("AI-OS"),
                "companion should mention AI-OS: {message}"
            );
        }
    }

    // ── Communication Philosophy: no internal terminology leaks ──────────

    const FORBIDDEN_TERMS: &[&str] = &[
        "world model",
        "entity",
        "relationship",
        "confidence",
        "attention score",
        "cognitive loop",
        "prediction engine",
        "memory graph",
        "strength=",
        "threshold",
        "signal strength",
        "Noted:",
        "Continuing where we left off",
        "Escalation:",
        "I've noted you",
        "I see you're working on",
        "(\n",
        "detected (strength=",
    ];

    fn assert_no_internal_terms(msg: &str, context: &str) {
        let lower = msg.to_lowercase();
        for term in FORBIDDEN_TERMS {
            assert!(
                !lower.contains(term),
                "[{context}] message leaked internal term '{term}': {msg}"
            );
        }
    }

    #[test]
    fn test_stagnation_message_no_internal_leaks() {
        let types = ["project", "task", "person", "user", "skill", "custom_type"];
        for entity_type in &types {
            let entity = Entity::new(entity_type, "AI-OS Project");
            let msg = CognitiveLoopService::stagnation_message(&entity);
            assert_no_internal_terms(&msg, &format!("stagnation_message({entity_type})"));
        }
    }

    #[tokio::test]
    async fn test_decide_greeting_no_internal_leaks_person_first() {
        let store = Arc::new(InMemoryWorldModelStore::new());
        let result = StructuredWorldUpdate {
            entities: vec![WorldEntity {
                name: "Alice".into(),
                entity_type: "person".into(),
                properties: HashMap::new(),
                confidence: 0.9,
                importance: 0.7,
            }],
            relationships: vec![],
            state_changes: vec![],
            new_observations: vec![],
            open_questions: vec![],
            possible_hypotheses: vec![],
        };
        let mut loop_svc = make_loop(store, mock_understanding(result));
        let decision = loop_svc.cycle("Hi, I'm Alice").await;
        if let Decision::Communicate { message, .. } = &decision {
            assert_no_internal_terms(message, "person first greeting");
        }
    }

    #[tokio::test]
    async fn test_decide_greeting_no_internal_leaks_person_returning() {
        let store = Arc::new(InMemoryWorldModelStore::new());
        store
            .insert_entity(Entity::new("person", "Alice").with_importance(0.7))
            .await;
        let result = StructuredWorldUpdate {
            entities: vec![WorldEntity {
                name: "Alice".into(),
                entity_type: "person".into(),
                properties: HashMap::new(),
                confidence: 0.9,
                importance: 0.7,
            }],
            relationships: vec![],
            state_changes: vec![],
            new_observations: vec![],
            open_questions: vec![],
            possible_hypotheses: vec![],
        };
        let mut loop_svc = make_loop(store, mock_understanding(result));
        let decision = loop_svc.cycle("Alice here again").await;
        if let Decision::Communicate { message, .. } = &decision {
            assert_no_internal_terms(message, "person returning greeting");
        }
    }

    #[tokio::test]
    async fn test_decide_greeting_no_internal_leaks_project_first() {
        let store = Arc::new(InMemoryWorldModelStore::new());
        let result = StructuredWorldUpdate {
            entities: vec![WorldEntity {
                name: "Website SEO".into(),
                entity_type: "project".into(),
                properties: HashMap::new(),
                confidence: 0.9,
                importance: 0.6,
            }],
            relationships: vec![],
            state_changes: vec![],
            new_observations: vec![],
            open_questions: vec![],
            possible_hypotheses: vec![],
        };
        let mut loop_svc = make_loop(store, mock_understanding(result));
        let decision = loop_svc.cycle("Working on my website SEO").await;
        if let Decision::Communicate { message, .. } = &decision {
            assert_no_internal_terms(message, "project first greeting");
        }
    }

    #[tokio::test]
    async fn test_decide_no_internal_leaks_observation_only() {
        let store = Arc::new(InMemoryWorldModelStore::new());
        let result = StructuredWorldUpdate {
            entities: vec![],
            relationships: vec![],
            state_changes: vec![],
            new_observations: vec!["User mentioned something".into()],
            open_questions: vec![],
            possible_hypotheses: vec![],
        };
        let mut loop_svc = make_loop(store, mock_understanding(result));
        let decision = loop_svc.cycle("Just thinking out loud").await;
        if let Decision::Communicate { message, .. } = &decision {
            assert_no_internal_terms(message, "observation-only");
        }
    }

    // ── Runtime grounding: never claim success without a confirmed result ──

    /// Coordinator that records the respond prompt so tests can assert the
    /// runtime grounding block reached the model.
    #[derive(Debug)]
    struct RecordingCoordinator {
        plan_json: &'static str,
        memory_json: &'static str,
        reply: &'static str,
        captured_prompt: Arc<std::sync::Mutex<Option<String>>>,
    }

    impl RecordingCoordinator {
        fn new(
            plan_json: &'static str,
            memory_json: &'static str,
            reply: &'static str,
        ) -> Arc<Self> {
            Arc::new(Self {
                plan_json,
                memory_json,
                reply,
                captured_prompt: Arc::new(std::sync::Mutex::new(None)),
            })
        }

        fn captured(&self) -> String {
            self.captured_prompt
                .lock()
                .unwrap()
                .clone()
                .expect("respond prompt should have been captured")
        }
    }

    #[async_trait::async_trait]
    impl intelligence_core::traits::IntelligenceCoordinator for RecordingCoordinator {
        async fn request(
            &self,
            request: intelligence_core::types::ModelRequest,
        ) -> Result<intelligence_core::types::ModelResponse, intelligence_core::ModelError>
        {
            let input = match &request.input {
                intelligence_core::types::ModelInput::Text(t) => t.as_str(),
                _ => "",
            };
            let content = if input.contains("planning engine") {
                self.plan_json
            } else if input.contains("memory evaluator") {
                self.memory_json
            } else {
                // respond prompt — capture for assertion
                if let Ok(mut guard) = self.captured_prompt.lock() {
                    *guard = Some(input.to_string());
                }
                self.reply
            };
            Ok(intelligence_core::types::ModelResponse {
                request_id: intelligence_core::types::RequestId::new(),
                model_id: intelligence_core::types::ModelId::new(),
                content: content.into(),
                usage: intelligence_core::types::TokenUsage {
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
            _request: intelligence_core::types::ModelRequest,
        ) -> Result<
            Box<dyn futures::Stream<Item = intelligence_core::types::StreamChunk> + Send>,
            intelligence_core::ModelError,
        > {
            unimplemented!("stream not used in tests")
        }

        async fn embed(
            &self,
            _texts: &[String],
        ) -> Result<Vec<intelligence_core::types::Embedding>, intelligence_core::ModelError>
        {
            unimplemented!("embed not used in tests")
        }

        async fn health(&self) -> Result<(), intelligence_core::ModelError> {
            Ok(())
        }

        async fn pipeline_stats(
            &self,
        ) -> Result<intelligence_core::types::IntelligenceStats, intelligence_core::ModelError>
        {
            unimplemented!("stats not used in tests")
        }

        async fn conversation(
            &self,
            _id: &intelligence_core::types::ConversationId,
        ) -> Result<Vec<intelligence_core::types::ModelResponse>, intelligence_core::ModelError>
        {
            unimplemented!("conversation not used in tests")
        }
    }

    #[tokio::test]
    async fn test_email_send_failure_is_grounded_in_respond_prompt() {
        use crate::planner::runtime_executor::RuntimeAwareExecutor;
        use ai_os_runtime_manager::RuntimeManager;

        let store = Arc::new(InMemoryWorldModelStore::new());

        // Runtime manager with NO registered runtime: email.send dispatch
        // must fail with capability_not_found — never report success.
        let manager = Arc::new(RuntimeManager::new());
        let executor = RuntimeAwareExecutor::new(manager);

        let coordinator = RecordingCoordinator::new(
            r#"{"actions":[{"type":"email.send","topics":["admin@example.com","Hi","Body"],"reason":"user requested"},{"type":"respond","topics":[],"reason":"reply"}]}"#,
            r#"[{"store":false}]"#,
            "reply",
        );
        let mut loop_svc = CognitiveLoopService::new(
            store,
            mock_understanding(super::empty_understanding()),
            coordinator.clone() as Arc<dyn IntelligenceCoordinator>,
        );
        loop_svc.set_runtime(Arc::new(executor)).await;
        loop_svc.register_runtime_capability("email.send", "Send an email");

        let _decision = loop_svc.cycle("send an email to admin@example.com").await;

        let captured = coordinator.captured();
        // The respond prompt must carry the failed dispatch result and the
        // honesty rule — so the model cannot claim "email sent successfully".
        assert!(
            captured.contains("email.send: FAILED"),
            "respond prompt must contain the FAILED dispatch result, got: {captured}"
        );
        assert!(
            captured.contains("could not be completed"),
            "respond prompt must carry the honesty instruction, got: {captured}"
        );
    }

    #[tokio::test]
    async fn test_email_send_success_is_grounded_in_respond_prompt() {
        use crate::planner::runtime_executor::RuntimeAwareExecutor;
        use ai_os_runtime_manager::RuntimeManager;

        let store = Arc::new(InMemoryWorldModelStore::new());

        // Runtime manager with a runtime that claims email.send and returns
        // a successful result.
        let manager = Arc::new(RuntimeManager::new());
        #[derive(Debug)]
        struct OkRuntime;
        #[async_trait::async_trait]
        impl ai_os_runtime_api::Runtime for OkRuntime {
            fn id(&self) -> ai_os_runtime_api::RuntimeId {
                ai_os_runtime_api::RuntimeId("ok".into())
            }
            async fn initialize(&self) -> Result<(), ai_os_runtime_api::RuntimeError> {
                Ok(())
            }
            async fn capabilities(&self) -> Vec<ai_os_runtime_api::Capability> {
                vec![ai_os_runtime_api::Capability {
                    id: ai_os_runtime_api::CapabilityId::new("email.send"),
                    name: "send email".into(),
                    description: "Send an email".into(),
                    input_schema: serde_json::json!({"type": "object"}),
                    output_schema: None,
                    side_effects: vec![],
                    metadata: Default::default(),
                }]
            }
            async fn capability(
                &self,
                id: &ai_os_runtime_api::CapabilityId,
            ) -> Option<ai_os_runtime_api::Capability> {
                (id.as_str() == "email.send").then(|| ai_os_runtime_api::Capability {
                    id: ai_os_runtime_api::CapabilityId::new("email.send"),
                    name: "send email".into(),
                    description: "Send an email".into(),
                    input_schema: serde_json::json!({"type": "object"}),
                    output_schema: None,
                    side_effects: vec![],
                    metadata: Default::default(),
                })
            }
            async fn execute(
                &self,
                action: ai_os_runtime_api::Action,
            ) -> Result<ai_os_runtime_api::ActionResult, ai_os_runtime_api::RuntimeError>
            {
                Ok(ai_os_runtime_api::ActionResult::succeeded(
                    action.action_id,
                    serde_json::json!({"status": "queued"}),
                ))
            }
            async fn observe(&self) -> Vec<ai_os_runtime_api::Observation> {
                Vec::new()
            }
            async fn subscribe(
                &self,
            ) -> Result<
                tokio::sync::mpsc::Receiver<ai_os_runtime_api::RuntimeEvent>,
                ai_os_runtime_api::RuntimeError,
            > {
                Err(ai_os_runtime_api::RuntimeError::SubscriptionUnsupported)
            }
            async fn health(&self) -> ai_os_runtime_api::RuntimeHealth {
                ai_os_runtime_api::RuntimeHealth::Ready
            }
        }
        manager
            .register(std::sync::Arc::new(OkRuntime))
            .await
            .unwrap();
        manager.initialize().await.unwrap();
        let executor = RuntimeAwareExecutor::new(manager);

        let coordinator = RecordingCoordinator::new(
            r#"{"actions":[{"type":"email.send","topics":["admin@example.com","Hi","Body"],"reason":"user requested"},{"type":"respond","topics":[],"reason":"reply"}]}"#,
            r#"[{"store":false}]"#,
            "reply",
        );
        let mut loop_svc = CognitiveLoopService::new(
            store,
            mock_understanding(super::empty_understanding()),
            coordinator.clone() as Arc<dyn IntelligenceCoordinator>,
        );
        loop_svc.set_runtime(Arc::new(executor)).await;
        loop_svc.register_runtime_capability("email.send", "Send an email");

        let _decision = loop_svc.cycle("send an email to admin@example.com").await;

        let captured = coordinator.captured();
        assert!(
            captured.contains("email.send: SUCCEEDED"),
            "respond prompt must contain the SUCCEEDED result, got: {captured}"
        );
    }
}
