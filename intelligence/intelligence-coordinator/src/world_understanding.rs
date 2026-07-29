use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use intelligence_core::traits::IntelligenceCoordinator;
use intelligence_core::types::{CapabilityKind, ModelInput, ModelRequest, RequestId};

/// Structured output from AI-driven World Understanding.
///
/// Produced by [`WorldUnderstandingService`] from a raw observation.
/// Contains everything needed to update the World Model without
/// heuristic extraction or rule-based parsing.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StructuredWorldUpdate {
    /// Entities discovered or confirmed in the observation.
    pub entities: Vec<WorldEntity>,
    /// Relationships between entities.
    pub relationships: Vec<WorldRelationship>,
    /// State changes affecting any entity.
    pub state_changes: Vec<StateChange>,
    /// New observations worth recording.
    pub new_observations: Vec<String>,
    /// Open questions the understanding raises.
    pub open_questions: Vec<String>,
    /// Possible hypotheses worth exploring.
    pub possible_hypotheses: Vec<String>,
}

/// An entity in the structured world update.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WorldEntity {
    /// Human-readable name.
    pub name: String,
    /// Open-ended type (e.g. "person", "device", "project", "skill").
    pub entity_type: String,
    /// Arbitrary key-value properties.
    #[serde(default)]
    pub properties: HashMap<String, String>,
    /// Confidence in this entity's existence (0.0–1.0).
    #[serde(default = "default_confidence")]
    pub confidence: f64,
    /// Importance of this entity to the user's world (0.0–1.0).
    #[serde(default)]
    pub importance: f64,
}

/// A relationship between two entities.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WorldRelationship {
    /// Name of the source entity.
    pub source: String,
    /// Name of the target entity.
    pub target: String,
    /// Type of relationship (e.g. "owns", "leads", "learning").
    pub relationship_type: String,
    /// Confidence in this relationship (0.0–1.0).
    #[serde(default = "default_confidence")]
    pub confidence: f64,
    /// Strength of the relationship (0.0–1.0).
    #[serde(default)]
    pub weight: f64,
}

/// A state change affecting an entity.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StateChange {
    /// Name of the entity that changed.
    pub entity_name: String,
    /// What changed (e.g. "status", "mood", "progress").
    pub attribute: String,
    /// Previous value.
    pub old_value: Option<String>,
    /// New value.
    pub new_value: String,
}

fn default_confidence() -> f64 {
    0.5
}

/// AI-driven World Understanding capability.
///
/// Given a raw observation (conversation, system event, sensor reading, etc.),
/// it uses the configured Intelligence provider to produce structured
/// understanding. No heuristics, no regex, no rule-based extraction.
///
/// Its sole responsibility is understanding. It does not plan, execute,
/// prioritise, or reflect.
#[derive(Clone)]
pub struct WorldUnderstandingService {
    coordinator: Arc<dyn IntelligenceCoordinator>,
}

impl fmt::Debug for WorldUnderstandingService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WorldUnderstandingService").finish()
    }
}

impl WorldUnderstandingService {
    /// Create a new service backed by the given Intelligence coordinator.
    pub fn new(coordinator: Arc<dyn IntelligenceCoordinator>) -> Self {
        Self { coordinator }
    }

    /// Produce structured understanding from a raw observation.
    ///
    /// The LLM is instructed to return ONLY valid JSON matching the
    /// [`StructuredWorldUpdate`] schema. If the initial response fails to
    /// parse, it retries once with a repair instruction.
    pub async fn understand(&self, observation: &str) -> Result<StructuredWorldUpdate, String> {
        let prompt = format!(
            r#"You are a world understanding engine. Your only job is to observe and understand.

Given the observation below, produce a structured JSON description of what it means about the user's world.

Rules:
- Return ONLY valid JSON. No markdown, no code fences, no commentary.
- Entity types are open-ended. Use whatever type naturally fits.
- Confidence reflects how certain you are (0.0–1.0).
- Importance reflects how significant this is to the user (0.0–1.0).
- If nothing meaningful can be extracted, return empty arrays.
- Do NOT fabricate entities or relationships you are not confident about.
- Relationships must reference entity names that exist in the "entities" array.

Output JSON schema:
{{
  "entities": [
    {{
      "name": "entity name",
      "entity_type": "open-ended type",
      "properties": {{}},
      "confidence": 0.0–1.0,
      "importance": 0.0–1.0
    }}
  ],
  "relationships": [
    {{
      "source": "source entity name",
      "target": "target entity name",
      "relationship_type": "type of relationship",
      "confidence": 0.0–1.0,
      "weight": 0.0–1.0
    }}
  ],
  "state_changes": [
    {{
      "entity_name": "entity name",
      "attribute": "what changed",
      "old_value": null,
      "new_value": "new value"
    }}
  ],
  "new_observations": [],
  "open_questions": [],
  "possible_hypotheses": []
}}

Observation:
{observation}"#,
            observation = observation
        );

        let request = ModelRequest {
            request_id: RequestId::new(),
            capability: CapabilityKind::Chat,
            model_id: None,
            input: ModelInput::Text(prompt),
            parameters: HashMap::new(),
            temperature: Some(0.3),
            top_p: None,
            max_tokens: Some(2048),
            stream: false,
        };

        let response = self
            .coordinator
            .request(request)
            .await
            .map_err(|e| format!("LLM request failed: {e}"))?;

        let cleaned = strip_markdown_json(&response.content);

        match serde_json::from_str::<StructuredWorldUpdate>(cleaned) {
            Ok(update) => Ok(update),
            Err(e) => {
                let repair_prompt = format!(
                    r#"Your previous response was not valid JSON matching the required schema.

Error: {e}

Raw response: {cleaned}

Respond with ONLY valid JSON matching this schema:
{{
  "entities": [{{ "name": "...", "entity_type": "...", "properties": {{}}, "confidence": 0.5, "importance": 0.5 }}],
  "relationships": [{{ "source": "...", "target": "...", "relationship_type": "...", "confidence": 0.5, "weight": 0.5 }}],
  "state_changes": [],
  "new_observations": [],
  "open_questions": [],
  "possible_hypotheses": []
}}

Observation was: {observation}"#,
                    e = e,
                    cleaned = cleaned,
                    observation = observation,
                );

                let repair_request = ModelRequest {
                    request_id: RequestId::new(),
                    capability: CapabilityKind::Chat,
                    model_id: None,
                    input: ModelInput::Text(repair_prompt),
                    parameters: HashMap::new(),
                    temperature: Some(0.1),
                    top_p: None,
                    max_tokens: Some(2048),
                    stream: false,
                };

                let repair_response = self
                    .coordinator
                    .request(repair_request)
                    .await
                    .map_err(|e| format!("LLM repair request failed: {e}"))?;

                let repaired = strip_markdown_json(&repair_response.content);
                serde_json::from_str::<StructuredWorldUpdate>(repaired).map_err(|e| {
                    format!("World understanding failed after retry: {e}\nraw: {repaired}")
                })
            }
        }
    }
}

fn strip_markdown_json(raw: &str) -> &str {
    let trimmed = raw.trim();
    if let Some(inner) = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))
    {
        let trimmed_inner = inner.trim();
        if let Some(end) = trimmed_inner.strip_suffix("```") {
            return end.trim();
        }
        return trimmed_inner;
    }
    trimmed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_markdown_json_fences() {
        let input = "```json\n{\"entities\": []}\n```";
        let result = strip_markdown_json(input);
        assert_eq!(result, "{\"entities\": []}");
    }

    #[test]
    fn test_strip_markdown_json_no_fences() {
        let input = "{\"entities\": []}";
        let result = strip_markdown_json(input);
        assert_eq!(result, "{\"entities\": []}");
    }

    #[test]
    fn test_strip_markdown_json_incomplete() {
        let input = "```json\n{\"entities\": []}";
        let result = strip_markdown_json(input);
        assert_eq!(result, "{\"entities\": []}");
    }

    #[test]
    fn test_structured_world_update_roundtrip_empty() {
        let update = StructuredWorldUpdate {
            entities: vec![],
            relationships: vec![],
            state_changes: vec![],
            new_observations: vec![],
            open_questions: vec![],
            possible_hypotheses: vec![],
        };
        let json = serde_json::to_string(&update).unwrap();
        let deserialized: StructuredWorldUpdate = serde_json::from_str(&json).unwrap();
        assert!(deserialized.entities.is_empty());
        assert!(deserialized.relationships.is_empty());
    }

    #[test]
    fn test_structured_world_update_roundtrip_with_data() {
        let update = StructuredWorldUpdate {
            entities: vec![WorldEntity {
                name: "Alice".into(),
                entity_type: "person".into(),
                properties: HashMap::new(),
                confidence: 0.9,
                importance: 0.8,
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
        let json = serde_json::to_string(&update).unwrap();
        let deserialized: StructuredWorldUpdate = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.entities.len(), 1);
        assert_eq!(deserialized.entities[0].name, "Alice");
        assert_eq!(deserialized.relationships.len(), 1);
        assert_eq!(deserialized.relationships[0].relationship_type, "owns");
    }

    #[test]
    fn test_deserialize_with_defaults_for_missing_fields() {
        let json = r#"{
            "entities": [{"name": "Alice", "entity_type": "person"}],
            "relationships": [],
            "state_changes": [],
            "new_observations": [],
            "open_questions": [],
            "possible_hypotheses": []
        }"#;
        let update: StructuredWorldUpdate = serde_json::from_str(json).unwrap();
        assert_eq!(update.entities[0].confidence, 0.5);
        assert_eq!(update.entities[0].importance, 0.0);
        assert!(update.entities[0].properties.is_empty());
    }

    #[test]
    fn test_world_entity_properties_roundtrip() {
        let mut props = HashMap::new();
        props.insert("status".into(), "active".into());
        let entity = WorldEntity {
            name: "Project X".into(),
            entity_type: "project".into(),
            properties: props,
            confidence: 0.8,
            importance: 0.7,
        };
        let json = serde_json::to_string(&entity).unwrap();
        let deserialized: WorldEntity = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.properties.get("status").unwrap(), "active");
    }

    #[test]
    fn test_state_change_roundtrip() {
        let sc = StateChange {
            entity_name: "Alice".into(),
            attribute: "mood".into(),
            old_value: Some("tired".into()),
            new_value: "energetic".into(),
        };
        let json = serde_json::to_string(&sc).unwrap();
        let deserialized: StateChange = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.entity_name, "Alice");
        assert_eq!(deserialized.old_value, Some("tired".into()));
        assert_eq!(deserialized.new_value, "energetic");
    }
}
