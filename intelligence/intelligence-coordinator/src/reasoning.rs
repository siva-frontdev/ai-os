use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use async_trait::async_trait;
use brain_core::traits::ReasoningService;
use brain_core::types::{
    Confidence, ExtractedEntity, GoalPriority, InferredConstraint, ReasoningResult,
};
use brain_core::BrainResult;
use intelligence_core::traits::IntelligenceCoordinator;
use intelligence_core::types::{CapabilityKind, ModelInput, ModelRequest, RequestId};

const SYSTEM_INSTRUCTION: &str = "\
You are an AI reasoning engine. Your purpose is to interpret user requests \
and return structured JSON describing the user's goal.

Rules:
- Return ONLY valid JSON. No markdown, no code fences, no commentary.
- Every field must be present. Use null or empty array for missing values.

Required JSON structure:
{
  \"summary\": \"one-sentence summary of what the user wants\",
  \"intent\": \"short_label_for_the_intent\",
  \"confidence\": 0.0-1.0,
  \"observations\": [\"relevant_context_from_request\"],
  \"entities\": [
    { \"name\": \"entity_name\", \"entity_type\": \"app|url|file|value|window|search_query\", \"value\": \"extracted_value\" }
  ],
  \"inferred_constraints\": [],
  \"assumptions\": [],
  \"hypotheses\": [],
  \"selected_hypothesis\": null,
  \"suggested_goal\": \"actionable goal description\",
  \"suggested_priority\": \"Low|Normal|High|Critical\",
  \"required_capabilities\": [\"capability.string\"],
  \"explanation\": \"why you interpreted it this way\",
  \"metadata\": {}
}

User request:";

const REPAIR_INSTRUCTION: &str = "\
Your previous response was not valid JSON. Please respond with ONLY valid JSON \
matching the schema described earlier. No markdown, no code fences, no extra text.";

const MAX_REPAIR_ATTEMPTS: u32 = 1;

#[derive(Clone)]
pub struct IntelligenceReasoningService {
    coordinator: Arc<dyn IntelligenceCoordinator>,
}

impl fmt::Debug for IntelligenceReasoningService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IntelligenceReasoningService").finish()
    }
}

impl IntelligenceReasoningService {
    pub fn new(coordinator: Arc<dyn IntelligenceCoordinator>) -> Self {
        Self { coordinator }
    }

    fn parse_priority(s: &str) -> GoalPriority {
        match s.trim().to_lowercase().as_str() {
            "low" => GoalPriority::Low,
            "high" => GoalPriority::High,
            "critical" => GoalPriority::Critical,
            _ => GoalPriority::Normal,
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
}

#[async_trait]
impl ReasoningService for IntelligenceReasoningService {
    async fn reason(&self, input: &str) -> BrainResult<ReasoningResult> {
        let mut attempt = 0u32;
        loop {
            let prompt = if attempt == 0 {
                format!("{SYSTEM_INSTRUCTION}\n{input}")
            } else {
                format!("{REPAIR_INSTRUCTION}\n\nYour previous attempt was for:\n{input}")
            };

            let request = ModelRequest {
                request_id: RequestId::new(),
                capability: CapabilityKind::Chat,
                model_id: None,
                input: ModelInput::Text(prompt),
                parameters: HashMap::new(),
                temperature: Some(0.1),
                top_p: None,
                max_tokens: Some(2048),
                stream: false,
            };

            let response =
                self.coordinator.request(request).await.map_err(|e| {
                    brain_core::errors::BrainError::ModelProviderError(e.to_string())
                })?;

            match Self::parse_response(&response.content) {
                Ok(result) => return Ok(result),
                Err(e) => {
                    attempt += 1;
                    if attempt > MAX_REPAIR_ATTEMPTS {
                        return Err(brain_core::errors::BrainError::ModelProviderError(format!(
                            "reasoning failed after {} attempts: {e}",
                            attempt,
                        )));
                    }
                }
            }
        }
    }
}

impl IntelligenceReasoningService {
    fn parse_response(raw: &str) -> BrainResult<ReasoningResult> {
        let cleaned = Self::strip_markdown_json(raw);

        #[derive(serde::Deserialize)]
        struct RawResult {
            summary: Option<String>,
            intent: Option<String>,
            confidence: Option<f64>,
            observations: Option<Vec<String>>,
            entities: Option<Vec<RawEntity>>,
            inferred_constraints: Option<Vec<RawConstraint>>,
            assumptions: Option<Vec<String>>,
            hypotheses: Option<Vec<String>>,
            selected_hypothesis: Option<String>,
            suggested_goal: Option<String>,
            suggested_priority: Option<String>,
            required_capabilities: Option<Vec<String>>,
            explanation: Option<String>,
            metadata: Option<HashMap<String, String>>,
        }

        #[derive(serde::Deserialize)]
        struct RawEntity {
            name: Option<String>,
            entity_type: Option<String>,
            value: Option<String>,
        }

        #[derive(serde::Deserialize)]
        struct RawConstraint {
            description: Option<String>,
            constraint_type: Option<String>,
        }

        let raw: RawResult = serde_json::from_str(cleaned).map_err(|e| {
            brain_core::errors::BrainError::ModelProviderError(format!(
                "failed to parse LLM output as ReasoningResult JSON: {e}\nraw: {cleaned}"
            ))
        })?;

        let priority = raw
            .suggested_priority
            .as_deref()
            .map(Self::parse_priority)
            .unwrap_or(GoalPriority::Normal);

        let entities = raw
            .entities
            .unwrap_or_default()
            .into_iter()
            .map(|e| ExtractedEntity {
                name: e.name.unwrap_or_default(),
                entity_type: e.entity_type.unwrap_or_default(),
                value: e.value.unwrap_or_default(),
            })
            .collect();

        let constraints = raw
            .inferred_constraints
            .unwrap_or_default()
            .into_iter()
            .map(|c| InferredConstraint {
                description: c.description.unwrap_or_default(),
                constraint_type: c.constraint_type.unwrap_or_default(),
            })
            .collect();

        Ok(ReasoningResult {
            summary: raw.summary.unwrap_or_default(),
            intent: raw.intent.unwrap_or_default(),
            confidence: Confidence(raw.confidence.unwrap_or(0.5) as f32),
            observations: raw.observations.unwrap_or_default(),
            entities,
            inferred_constraints: constraints,
            assumptions: raw.assumptions.unwrap_or_default(),
            hypotheses: raw.hypotheses.unwrap_or_default(),
            selected_hypothesis: raw.selected_hypothesis,
            suggested_goal: raw.suggested_goal.unwrap_or_default(),
            suggested_priority: priority,
            required_capabilities: raw.required_capabilities.unwrap_or_default(),
            explanation: raw.explanation.unwrap_or_default(),
            metadata: raw.metadata.unwrap_or_default(),
        })
    }
}
