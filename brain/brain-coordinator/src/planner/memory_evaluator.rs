use std::collections::HashMap;
use std::sync::Arc;

use intelligence_coordinator::world_understanding::{StructuredWorldUpdate, WorldEntity};
use intelligence_core::traits::IntelligenceCoordinator;
use intelligence_core::types::{CapabilityKind, ModelInput, ModelRequest, RequestId};

/// Decision about whether a piece of understanding should be stored.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MemoryDecision {
    /// Whether to store this entity in the World Model.
    pub store: bool,
    /// Importance score (0.0–1.0) if storing.
    #[serde(default)]
    pub importance: f64,
    /// Confidence score (0.0–1.0) if storing.
    #[serde(default = "default_score")]
    pub confidence: f64,
    /// Human-readable reason for the decision.
    #[serde(default)]
    pub reason: String,
    /// Short summary of what was learned.
    #[serde(default)]
    pub summary: String,
}

fn default_score() -> f64 {
    0.5
}

/// AI-driven memory evaluation.
///
/// Decides whether observed entities represent durable knowledge
/// worth storing in the World Model. Replaces hardcoded entity
/// type classification with meaning-based reasoning.
///
/// The LLM evaluates each entity by its semantic content, not its
/// entity type name. "My favorite language is Rust" → store.
/// "Hi" → do not store.
/// "I'm working on AI-OS" → store.
/// "Good" → do not store.
#[derive(Debug)]
pub struct MemoryEvaluator {
    coordinator: Arc<dyn IntelligenceCoordinator>,
}

impl MemoryEvaluator {
    pub fn new(coordinator: Arc<dyn IntelligenceCoordinator>) -> Self {
        Self { coordinator }
    }

    /// Evaluate whether the understanding from an observation
    /// contains durable knowledge worth storing.
    ///
    /// Returns `None` when no entities merit storage (pure transient).
    /// Returns `Some(filtered_update)` with adjusted importance/confidence
    /// for entities the evaluator deems worth keeping.
    pub async fn evaluate(
        &self,
        observation: &str,
        update: &StructuredWorldUpdate,
    ) -> Option<StructuredWorldUpdate> {
        if update.entities.is_empty() {
            return None;
        }

        let decisions = self.evaluate_entities(observation, &update.entities).await;

        let stored_entities: Vec<WorldEntity> = update
            .entities
            .iter()
            .zip(decisions.iter())
            .filter(|(_, d)| d.store)
            .map(|(e, d)| {
                let mut stored = (*e).clone();
                stored.importance = d.importance;
                stored.confidence = d.confidence;
                stored
            })
            .collect();

        if stored_entities.is_empty() {
            return None;
        }

        Some(StructuredWorldUpdate {
            entities: stored_entities,
            ..update.clone()
        })
    }

    /// Get the LLM to evaluate each entity for storage worthiness.
    async fn evaluate_entities(
        &self,
        observation: &str,
        entities: &[WorldEntity],
    ) -> Vec<MemoryDecision> {
        if entities.is_empty() {
            return vec![];
        }

        let entity_descriptions: Vec<String> = entities
            .iter()
            .map(|e| {
                let props: Vec<String> = e
                    .properties
                    .iter()
                    .map(|(k, v)| format!("{k}={v}"))
                    .collect();
                let prop_str = if props.is_empty() {
                    String::new()
                } else {
                    format!(", properties: [{}]", props.join(", "))
                };
                format!(
                    "- name: \"{}\", type: \"{}\", importance: {:.2}, confidence: {:.2}{}",
                    e.name, e.entity_type, e.importance, e.confidence, prop_str
                )
            })
            .collect();

        let prompt = format!(
            r#"You are a memory evaluator. Your job is to decide which observed entities represent durable knowledge worth storing.

Given the user's message and the entities extracted from it, decide for each entity whether it should be stored in long-term memory.

Rules:
- Store entities that represent persistent facts: people, projects, skills, preferences, goals, routines, interests, meaningful events.
- Do NOT store transient interactions: greetings, filler, acknowledgements, small talk, noise, or non-information.
- Base your decision on semantic meaning, not entity type names.
- "My favorite language is Rust" → store (durable preference).
- "Hi" → do not store (transient).
- "I'm working on AI-OS" → store (durable project).
- "Good" → do not store (transient).
- "I'm tired" → do not store (transient state), unless it reveals a durable pattern.
- Set importance and confidence based on how significant and certain the knowledge is.

Respond with ONLY valid JSON: an array of objects with fields: store (bool), importance (0.0-1.0), confidence (0.0-1.0), reason (string), summary (string).

User message: {observation}

Entities:
{entities}"#,
            observation = observation,
            entities = entity_descriptions.join("\n"),
        );

        let request = ModelRequest {
            request_id: RequestId::new(),
            capability: CapabilityKind::Chat,
            model_id: None,
            input: ModelInput::Text(prompt),
            parameters: HashMap::new(),
            temperature: Some(0.2),
            top_p: None,
            max_tokens: Some(1024),
            stream: false,
        };

        match self.coordinator.request(request).await {
            Ok(response) => {
                let cleaned = response.content.trim().to_string();
                let cleaned = cleaned
                    .strip_prefix("```json")
                    .or_else(|| cleaned.strip_prefix("```"))
                    .unwrap_or(&cleaned);
                let cleaned = cleaned.strip_suffix("```").unwrap_or(cleaned).trim();
                match serde_json::from_str::<Vec<MemoryDecision>>(cleaned) {
                    Ok(decisions) => {
                        if decisions.len() == entities.len() {
                            return decisions;
                        }
                        tracing::warn!(
                            "Memory evaluator returned {} decisions for {} entities",
                            decisions.len(),
                            entities.len()
                        );
                    }
                    Err(e) => {
                        tracing::warn!("Memory evaluator parse failed: {e}");
                    }
                }
                // Fallback: conservative — store everything as-is
                entities
                    .iter()
                    .map(|e| MemoryDecision {
                        store: !is_obviously_transient(&e.name, &e.entity_type, observation),
                        importance: e.importance,
                        confidence: e.confidence,
                        reason: "fallback: evaluator parse failed".into(),
                        summary: String::new(),
                    })
                    .collect()
            }
            Err(e) => {
                tracing::warn!("Memory evaluator LLM request failed: {e}");
                entities
                    .iter()
                    .map(|e| MemoryDecision {
                        store: true,
                        importance: e.importance,
                        confidence: e.confidence,
                        reason: "fallback: LLM unavailable".into(),
                        summary: String::new(),
                    })
                    .collect()
            }
        }
    }
}

/// Conservative heuristic fallback used only when the LLM evaluator
/// is unavailable. Not a semantic rule — purely structural.
fn is_obviously_transient(name: &str, entity_type: &str, observation: &str) -> bool {
    let obs_trimmed = observation.trim();
    if obs_trimmed.len() < 3 {
        return true;
    }
    // Only trigger on the most obviously transient patterns
    let transient = [
        "hi", "hello", "hey", "bye", "goodbye", "ok", "okay", "sure", "thanks",
    ];
    let lower = obs_trimmed.to_lowercase();
    if transient.contains(&lower.as_str()) {
        return true;
    }
    // Fallback entity name check for obviously auto-generated entities
    if entity_type == "greeting" || entity_type == "filler" || entity_type == "acknowledgement" {
        return true;
    }
    false
}

impl Clone for MemoryEvaluator {
    fn clone(&self) -> Self {
        Self {
            coordinator: self.coordinator.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_obviously_transient_empty() {
        assert!(is_obviously_transient("", "person", ""));
        assert!(is_obviously_transient("Hi", "person", "hi"));
    }

    #[test]
    fn test_obviously_transient_greeting() {
        assert!(is_obviously_transient("User", "person", "hello"));
        assert!(is_obviously_transient("User", "person", "hey"));
        assert!(is_obviously_transient("User", "person", "bye"));
    }

    #[test]
    fn test_not_transient_for_substantive() {
        assert!(!is_obviously_transient(
            "Rust",
            "skill",
            "I'm learning Rust"
        ));
        assert!(!is_obviously_transient(
            "AI-OS",
            "project",
            "Working on AI-OS"
        ));
        assert!(!is_obviously_transient(
            "Alice",
            "person",
            "My name is Alice"
        ));
    }
}
