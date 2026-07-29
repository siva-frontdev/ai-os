use std::collections::VecDeque;
use std::sync::Arc;

use crate::attention::{AttentionDecision, AttentionEvaluator, AttentionOutcome};
use crate::evolution::EvolutionEngine;
use brain_core::types::Decision;
use intelligence_coordinator::world_understanding::WorldUnderstandingService;
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

/// The heart of the Continuous Cognitive Loop (ADR-0007).
///
/// Implements one complete cognitive cycle:
///   1. Observe — accept raw text input
///   2. Interpret — use AI to produce structured world understanding
///   3. Evolve — merge observations into the World Model via Evolution Engine
///   4. **Evaluate Attention** — determines whether reflection is worthwhile
///   5. Reflect — evaluate current state against recent changes (only if attention granted)
///   6. Decide — produce a `Decision` (Wait, Communicate, UpdateMemory, Execute)
///   7. Learn — simple feedback loop (placeholder)
///
/// **Continuous Life**: Every cycle is part of one ongoing existence.
/// Continuity emerges naturally from World Model queries — entities sorted
/// by importance and recency form the context of "what was happening."
///
/// Understanding comes from the Intelligence Platform, not from heuristics.
/// Evolution handles entity deduplication, relationship strengthening, and
/// confidence/importance updates.
/// Attention ensures the AI only reflects when something deserves it.
#[derive(Debug)]
pub struct CognitiveLoopService {
    store: Arc<dyn WorldModelStore>,
    evolution: EvolutionEngine,
    understanding: WorldUnderstandingService,
    attention: AttentionEvaluator,
    cycle_count: u64,
    reflection_log: VecDeque<ReflectionLogEntry>,
    last_communicated_at: Option<Timestamp>,
    consecutive_silent_ticks: u32,
}

impl CognitiveLoopService {
    pub fn new(
        world_model: Arc<dyn WorldModelStore>,
        understanding: WorldUnderstandingService,
    ) -> Self {
        Self {
            store: world_model.clone(),
            evolution: EvolutionEngine::new(world_model),
            understanding,
            attention: AttentionEvaluator::new(),
            cycle_count: 0,
            reflection_log: VecDeque::new(),
            last_communicated_at: None,
            consecutive_silent_ticks: 0,
        }
    }

    /// Run one full cognitive cycle on the given text observation.
    ///
    /// Every cycle is part of one continuous existence. The engine:
    ///   - Builds continuity context (what was happening?)
    ///   - Observes and interprets the new input
    ///   - Evolves the world model
    ///   - Evaluates attention
    ///   - Decides what to do (using continuity context)
    ///   - Records what happened for future cycles
    pub async fn cycle(&mut self, observation: &str) -> Decision {
        self.cycle_count += 1;

        // 1. Build continuity context — what was happening before this input?
        let context = self.build_context().await;
        let context_summary = Self::format_context_summary(&context);

        // 2. Observe (input already received)
        // 3. Interpret — AI-driven world understanding
        let understanding = match self.understanding.understand(observation).await {
            Ok(u) => u,
            Err(e) => {
                tracing::warn!("World understanding failed: {e}");
                return Decision::Wait;
            }
        };

        // 4. Evolve World Model (merge entities, create/strengthen relationships)
        let report = self.evolution.evolve(&understanding).await;

        // 5. Evaluate Attention — does this deserve reflection?
        let attention = self.attention.evaluate(&understanding, &report);

        let decision = match attention.outcome {
            // Silence — not worth reflecting
            AttentionOutcome::Ignore
            | AttentionOutcome::ObserveLater { .. }
            | AttentionOutcome::ReflectSoon { .. } => Decision::Wait,

            // Immediate attention — proceed to reflect and decide
            AttentionOutcome::ReflectNow => {
                // 6. Decide with continuity context
                self.decide(&understanding, &context, &context_summary)
            }

            // Specific actions — map directly to decisions
            AttentionOutcome::AskUser { question } => Decision::Communicate {
                recipient: "user".into(),
                message: question,
                reason: "AI needs user input to proceed".into(),
            },
            AttentionOutcome::Suggest { message } => Decision::Communicate {
                recipient: "user".into(),
                message,
                reason: "Attention-based proactive suggestion".into(),
            },
            AttentionOutcome::Notify { message } => Decision::Communicate {
                recipient: "user".into(),
                message,
                reason: "Attention-based notification".into(),
            },
            AttentionOutcome::Escalate { reason } => Decision::Communicate {
                recipient: "user".into(),
                message: reason,
                reason: "Attention escalation".into(),
            },
        };

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

    /// Format a human-readable summary from context entities.
    fn format_context_summary(entities: &[Entity]) -> String {
        if entities.is_empty() {
            return "Starting fresh.".into();
        }
        let parts: Vec<String> = entities.iter().map(|e| e.name.clone()).collect();
        format!("Thinking about {}", parts.join(", "))
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
        let context_summary = Self::format_context_summary(&context);
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
    /// Requires positive justification before communicating:
    ///
    /// Valid reasons:
    ///   - **Stagnation**: a high-importance entity has not been discussed recently
    ///   - **Sustained silence**: the AI has not communicated in 3+ ticks and
    ///     there are important entities worth checking on
    ///   - **High-importance with progress**: entity importance > 0.8
    ///     AND there's been a recent change (detected via attention signals)
    ///
    /// Context alone is NOT sufficient to communicate. The AI prefers
    /// silence when there is no specific reason to reach out.
    fn decide_on_context(
        &self,
        context_entities: &[Entity],
        context_summary: &str,
        attention: &AttentionDecision,
    ) -> Decision {
        if context_entities.is_empty() {
            return Decision::Wait;
        }

        // Check for stagnation as the dominant signal
        let stagnation_strength = attention
            .signals
            .iter()
            .find(|s| s.name == "stagnation")
            .map(|s| s.strength)
            .unwrap_or(0.0);

        // Cool-down: don't nag on consecutive ticks. After a tick communicates,
        // wait at least 2 silent ticks before reaching out again.
        // First-ever communication is allowed without cool-down.
        let is_first_ever = self.last_communicated_at.is_none();
        if !is_first_ever && self.consecutive_silent_ticks < 2 {
            return Decision::Wait;
        }

        // Stagnation of an important entity (≥2 days stale) → follow up
        if stagnation_strength > 0.15 {
            let entity = context_entities
                .iter()
                .max_by(|a, b| a.importance.partial_cmp(&b.importance).unwrap());
            if let Some(entity) = entity {
                let message = Self::stagnation_message(entity);
                return Decision::Communicate {
                    recipient: "user".into(),
                    message,
                    reason: format!("stagnation: {} stale for days", entity.name),
                };
            }
        }

        // High importance + sustained silence (no recent interaction) → check in
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
                reason: format!(
                    "high-importance {} after {} silent ticks",
                    entity.name, self.consecutive_silent_ticks
                ),
            };
        }

        // Nothing justifies communication → prefer silence
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
    ///
    /// Reflections are NOT stored in the World Model because they represent
    /// the AI's internal reasoning, not the user's world. Storing them as
    /// WM entities would pollute the continuity context with AI-internal
    /// records that compete with user knowledge.
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

    fn decide(
        &self,
        understanding: &intelligence_coordinator::world_understanding::StructuredWorldUpdate,
        context_entities: &[Entity],
        context_summary: &str,
    ) -> Decision {
        let has_continuity = !context_entities.is_empty();

        if let Some(entity) = understanding.entities.first() {
            let reason = format!("Detected {}: {}", entity.entity_type, entity.name);
            if matches!(entity.entity_type.as_str(), "person" | "user") {
                let message = if has_continuity {
                    format!(
                        "Hello {}! I was thinking about you. {}",
                        entity.name, context_summary
                    )
                } else {
                    "I don't need to understand your whole life today — we'll build that together over time. What's on your mind right now?".into()
                };
                return Decision::Communicate {
                    recipient: entity.name.clone(),
                    message,
                    reason,
                };
            } else {
                let message = if has_continuity {
                    format!(
                        "I noticed you're still working on {}. {}",
                        entity.name, context_summary
                    )
                } else {
                    format!(
                        "I noticed you're working on {}. What's the latest?",
                        entity.name
                    )
                };
                return Decision::Communicate {
                    recipient: "user".into(),
                    message,
                    reason,
                };
            }
        }

        if !understanding.relationships.is_empty() {
            let rel = &understanding.relationships[0];
            let reason = format!(
                "Relationship: {} → {} → {}",
                rel.source, rel.relationship_type, rel.target
            );
            return Decision::UpdateMemory {
                entity_name: rel.target.clone(),
                entity_type: "entity".into(),
                properties: vec![("related".into(), rel.relationship_type.clone())],
                reason,
            };
        }

        if !understanding.new_observations.is_empty() {
            let message = if has_continuity {
                "I'm following along.".into()
            } else {
                "I'll remember that.".into()
            };
            return Decision::Communicate {
                recipient: "user".into(),
                message,
                reason: "New observation recorded — continuing prior context".into(),
            };
        }

        if !understanding.state_changes.is_empty() {
            let sc = &understanding.state_changes[0];
            return Decision::UpdateMemory {
                entity_name: sc.entity_name.clone(),
                entity_type: "state".into(),
                properties: vec![(sc.attribute.clone(), sc.new_value.clone())],
                reason: format!(
                    "State change: {} {} → {}",
                    sc.entity_name, sc.attribute, sc.new_value
                ),
            };
        }

        Decision::Wait
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
    use memory_core::Timestamp;
    use memory_core::wm::Entity;
    use memory_storage::wm_store::InMemoryWorldModelStore;
    use memory_storage::wm_store::WorldModelStore;

    // Helper: create a CognitiveLoopService with a mock understanding service
    // that returns predetermined structured updates.
    fn make_loop(
        store: Arc<dyn WorldModelStore>,
        understanding: WorldUnderstandingService,
    ) -> CognitiveLoopService {
        CognitiveLoopService::new(store, understanding)
    }

    #[tokio::test]
    async fn test_wait_on_empty_understanding() {
        let store = Arc::new(memory_storage::wm_store::InMemoryWorldModelStore::new());
        let result = StructuredWorldUpdate {
            entities: vec![],
            relationships: vec![],
            state_changes: vec![],
            new_observations: vec![],
            open_questions: vec![],
            possible_hypotheses: vec![],
        };
        let empty = mock_understanding(result);
        let mut loop_svc = make_loop(store, empty);
        let decision = loop_svc.cycle("nothing here").await;
        assert!(matches!(decision, Decision::Wait));
        assert_eq!(loop_svc.cycle_count(), 1);
    }

    #[tokio::test]
    async fn test_person_triggers_communicate() {
        let store = Arc::new(memory_storage::wm_store::InMemoryWorldModelStore::new());
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
        // Person entity triggers high attention → ReflectNow → decide()
        // High-importance person produces Notify (Communicate)
        assert!(
            matches!(decision, Decision::Communicate { .. }),
            "person should produce Communicate, got {decision:?}"
        );
    }

    #[tokio::test]
    async fn test_relationship_requires_sufficient_attention() {
        let store = Arc::new(memory_storage::wm_store::InMemoryWorldModelStore::new());
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
        let mut loop_svc = make_loop(store, svc);
        let decision = loop_svc.cycle("I bought a MacBook").await;
        // A device entity + relationship without other strong signals
        // may not meet attention threshold → Wait
        // The entity is still evolved into the World Model regardless.
        assert!(
            matches!(decision, Decision::Wait),
            "weak signals should produce Wait, got {decision:?}"
        );
    }

    #[tokio::test]
    async fn test_trivial_observation_waits() {
        let store = Arc::new(memory_storage::wm_store::InMemoryWorldModelStore::new());
        let result = StructuredWorldUpdate {
            entities: vec![],
            relationships: vec![],
            state_changes: vec![],
            new_observations: vec!["User mentioned reading a book".into()],
            open_questions: vec![],
            possible_hypotheses: vec![],
        };
        let svc = mock_understanding(result);
        let mut loop_svc = make_loop(store, svc);
        let decision = loop_svc.cycle("reading a book").await;
        // A trivial observation without urgency/people/state change
        // gets deferred (ObserveLater/ReflectSoon → Wait)
        assert!(
            matches!(decision, Decision::Wait),
            "trivial observation should wait, got {decision:?}"
        );
    }

    #[tokio::test]
    async fn test_state_change_triggers_update_memory() {
        let store = Arc::new(memory_storage::wm_store::InMemoryWorldModelStore::new());
        let result = StructuredWorldUpdate {
            entities: vec![],
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
        let mut loop_svc = make_loop(store, svc);
        let decision = loop_svc.cycle("I am tired").await;
        match decision {
            Decision::UpdateMemory { ref properties, .. } => {
                assert!(properties.iter().any(|(k, v)| k == "mood" && v == "tired"));
            }
            other => panic!("expected UpdateMemory, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_cycle_stores_entities_in_world_model() {
        let store = Arc::new(memory_storage::wm_store::InMemoryWorldModelStore::new());
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
        let store = Arc::new(memory_storage::wm_store::InMemoryWorldModelStore::new());
        let svc = WorldUnderstandingService::new(Arc::new(
            // Use a mock coordinator that always errors
            MockFailingCoordinator,
        ));
        let mut loop_svc = make_loop(store, svc);
        let decision = loop_svc.cycle("anything").await;
        assert!(matches!(decision, Decision::Wait));
    }

    // ── Mock infrastructure ─────────────────────────────────

    fn mock_understanding(result: StructuredWorldUpdate) -> WorldUnderstandingService {
        WorldUnderstandingService::new(Arc::new(MockCoordinator(result)))
    }

    #[derive(Debug)]
    struct MockCoordinator(StructuredWorldUpdate);

    #[async_trait::async_trait]
    impl intelligence_core::traits::IntelligenceCoordinator for MockCoordinator {
        async fn request(
            &self,
            _request: intelligence_core::types::ModelRequest,
        ) -> Result<intelligence_core::types::ModelResponse, intelligence_core::ModelError>
        {
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

    #[derive(Debug)]
    struct MockFailingCoordinator;

    #[async_trait::async_trait]
    impl intelligence_core::traits::IntelligenceCoordinator for MockFailingCoordinator {
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

    // ── Daily Companion Tests ────────────────────────────────

    #[allow(dead_code)]
    fn empty_understanding() -> StructuredWorldUpdate {
        StructuredWorldUpdate {
            entities: vec![],
            relationships: vec![],
            state_changes: vec![],
            new_observations: vec![],
            open_questions: vec![],
            possible_hypotheses: vec![],
        }
    }

    #[tokio::test]
    async fn test_companion_tick_empty_wm_stays_silent() {
        let store = Arc::new(memory_storage::wm_store::InMemoryWorldModelStore::new());
        let svc = mock_understanding(empty_understanding());
        let mut loop_svc = make_loop(store, svc);
        let decision = loop_svc.tick().await;
        assert!(
            matches!(decision, Decision::Wait),
            "empty companion should stay silent, got {decision:?}"
        );
    }

    #[tokio::test]
    async fn test_companion_tick_stays_silent_with_fresh_entity() {
        let store = Arc::new(memory_storage::wm_store::InMemoryWorldModelStore::new());
        let entity = Entity::new("project", "AI-OS")
            .with_importance(0.85)
            .with_confidence(0.9);
        store.insert_entity(entity).await;

        let svc = mock_understanding(empty_understanding());
        let mut loop_svc = make_loop(store, svc);
        let decision = loop_svc.tick().await;
        assert!(
            matches!(decision, Decision::Wait),
            "fresh entity without stagnation should stay silent, got {decision:?}"
        );
    }

    #[tokio::test]
    async fn test_companion_reflection_recorded_in_log() {
        let store = Arc::new(memory_storage::wm_store::InMemoryWorldModelStore::new());
        let entity = Entity::new("project", "AI-OS")
            .with_importance(0.85)
            .with_confidence(0.9);
        store.insert_entity(entity).await;

        let svc = mock_understanding(empty_understanding());
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
        let store = Arc::new(memory_storage::wm_store::InMemoryWorldModelStore::new());
        let mut entity = Entity::new("project", "AI-OS")
            .with_importance(0.85)
            .with_confidence(0.9);
        let five_days_ago =
            Timestamp::from_nanos(Timestamp::now().as_nanos() - 5 * 86400 * 1_000_000_000);
        entity.updated_at = five_days_ago;
        store.insert_entity(entity).await;

        let svc = mock_understanding(empty_understanding());
        let mut loop_svc = make_loop(store, svc);
        let decision = loop_svc.tick().await;
        assert!(
            matches!(decision, Decision::Communicate { .. }),
            "stagnant high-importance project should trigger communication, got {decision:?}"
        );
    }

    #[tokio::test]
    async fn test_companion_daily_rhythm_natural_continuation() {
        let store = Arc::new(memory_storage::wm_store::InMemoryWorldModelStore::new());
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

        // Day 1 afternoon: user continues
        let afternoon_result = StructuredWorldUpdate {
            entities: vec![WorldEntity {
                name: "AI-OS".into(),
                entity_type: "project".into(),
                properties: {
                    let mut m = std::collections::HashMap::new();
                    m.insert("status".into(), "active".into());
                    m
                },
                confidence: 0.9,
                importance: 0.85,
            }],
            relationships: vec![],
            state_changes: vec![intelligence_coordinator::world_understanding::StateChange {
                entity_name: "AI-OS".into(),
                attribute: "status".into(),
                old_value: Some("started".into()),
                new_value: "active".into(),
            }],
            new_observations: vec![],
            open_questions: vec![],
            possible_hypotheses: vec![],
        };
        let _afternoon_svc = mock_understanding(afternoon_result);
        let _decision = loop_svc.cycle("Making progress on AI-OS").await;

        // The state change + continued interaction should produce something
        let entities = store.all_entities().await;
        let ai_os = entities.iter().find(|e| e.name == "AI-OS");
        assert!(ai_os.is_some(), "AI-OS should be in the World Model");
        if let Some(p) = ai_os {
            assert!(p.importance >= 0.8, "AI-OS importance should remain high");
        }
        // The decision may be Wait (if attention threshold isn't met) or Communicate
        // Either is valid — the key is the WM was updated
    }

    #[tokio::test]
    async fn test_companion_multiple_entities_stagnation_follow_up() {
        let store = Arc::new(memory_storage::wm_store::InMemoryWorldModelStore::new());

        // Entity stale for 6 days — stagnation ~0.51, triggers follow-up
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

        let svc = mock_understanding(empty_understanding());
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

    /// Terms that must never appear in user-facing companion messages.
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
        // Seed an entity so has_continuity is true
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
}
