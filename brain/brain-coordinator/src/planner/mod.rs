pub mod capabilities;
pub mod executor;
pub mod memory_evaluator;
pub mod runtime_executor;

use std::collections::HashMap;
use std::sync::Arc;

pub use capabilities::{Capability, CapabilityRegistry};
pub use executor::ActionExecutor;
pub use memory_evaluator::MemoryEvaluator;
pub use runtime_executor::{
    IN_MEMORY_ACTIONS, RuntimeAwareExecutor, is_in_memory_action, planned_to_runtime_action,
};

use intelligence_core::traits::IntelligenceCoordinator;
use intelligence_core::types::{CapabilityKind, ModelInput, ModelRequest, RequestId};

/// An action the Planner has decided to execute.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PlannedAction {
    #[serde(rename = "type")]
    pub action_type: String,
    #[serde(default)]
    pub topics: Vec<String>,
    #[serde(default)]
    pub reason: String,
}

/// The full plan returned by the Planner.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Plan {
    pub actions: Vec<PlannedAction>,
}

impl Plan {
    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }
}

/// The Planner decides what actions are needed based on the
/// user message, recent conversation, world model context,
/// and available capabilities.
///
/// The Planner NEVER generates the final response. It only
/// creates an execution plan. Rust executes the plan.
#[derive(Debug)]
pub struct Planner {
    coordinator: Arc<dyn IntelligenceCoordinator>,
    capabilities: CapabilityRegistry,
}

impl Planner {
    pub fn new(coordinator: Arc<dyn IntelligenceCoordinator>) -> Self {
        Self {
            coordinator,
            capabilities: CapabilityRegistry::default(),
        }
    }

    pub fn new_with_capabilities(
        coordinator: Arc<dyn IntelligenceCoordinator>,
        capabilities: CapabilityRegistry,
    ) -> Self {
        Self {
            coordinator,
            capabilities,
        }
    }

    pub fn capabilities(&self) -> &CapabilityRegistry {
        &self.capabilities
    }

    pub fn capabilities_mut(&mut self) -> &mut CapabilityRegistry {
        &mut self.capabilities
    }

    /// Plan what actions to take in response to a user message.
    pub async fn plan(
        &self,
        user_message: &str,
        recent_conversation: &str,
        memory_context: &str,
        has_prior_knowledge: bool,
    ) -> Plan {
        let caps: Vec<String> = self
            .capabilities
            .list()
            .iter()
            .map(|c| format!("- {}: {}", c.name, c.description))
            .collect();

        let prompt = format!(
            r#"You are a planning engine. Your job is to decide what actions the system should take.

Given the user's message and available context, create a plan using only the capabilities listed below.

Rules:
- Return ONLY valid JSON matching the schema: {{ "actions": [{{ "type": "...", "topics": [...], "reason": "..." }}] }}
- Never generate the final response text. Only plan actions.
- Use "respond" for the final action when a response is needed.
- Use "search_memory" when the user asks about what you know, remember, or should know.
- Use "search_recent_conversation" when the user refers to something just discussed.
- Use "ask_clarification" when the user's intent is genuinely ambiguous.
- Use "ignore" when the message is empty or trivial.
- Use "observe" when the user provides factual information without expecting a response.
- Topics should be specific search terms or subjects.
- The "respond" action is always the last action in the plan.

Available capabilities:
{caps}

Recent conversation:
{recent}

Memory context:
{memory}

Has prior knowledge: {has_knowledge}

User message: {message}"#,
            caps = caps.join("\n"),
            recent = if recent_conversation.is_empty() {
                "(no recent conversation)"
            } else {
                recent_conversation
            },
            memory = if memory_context.is_empty() {
                "(no stored knowledge)"
            } else {
                memory_context
            },
            has_knowledge = has_prior_knowledge,
            message = user_message,
        );

        let request = ModelRequest {
            request_id: RequestId::new(),
            capability: CapabilityKind::Chat,
            model_id: None,
            input: ModelInput::Text(prompt),
            parameters: HashMap::new(),
            temperature: Some(0.2),
            top_p: None,
            max_tokens: Some(512),
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
                match serde_json::from_str::<Plan>(cleaned) {
                    Ok(plan) => {
                        if plan.is_empty() {
                            tracing::info!("Planner returned empty plan, defaulting to respond");
                            return Plan {
                                actions: vec![PlannedAction {
                                    action_type: "respond".into(),
                                    topics: vec![],
                                    reason: "empty plan fallback".into(),
                                }],
                            };
                        }
                        plan
                    }
                    Err(e) => {
                        tracing::warn!("Planner parse failed: {e}, defaulting to respond");
                        Plan {
                            actions: vec![PlannedAction {
                                action_type: "respond".into(),
                                topics: vec![],
                                reason: format!("parse error: {e}"),
                            }],
                        }
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Planner LLM request failed: {e}, defaulting to respond");
                Plan {
                    actions: vec![PlannedAction {
                        action_type: "respond".into(),
                        topics: vec![],
                        reason: format!("LLM error: {e}"),
                    }],
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_planner(result: Plan) -> Planner {
        #[derive(Debug)]
        struct MockPlannerCoordinator(Plan);

        #[async_trait::async_trait]
        impl IntelligenceCoordinator for MockPlannerCoordinator {
            async fn request(
                &self,
                _request: ModelRequest,
            ) -> Result<intelligence_core::types::ModelResponse, intelligence_core::ModelError>
            {
                let json = serde_json::to_string(&self.0).unwrap();
                Ok(intelligence_core::types::ModelResponse {
                    request_id: RequestId::new(),
                    model_id: intelligence_core::types::ModelId::new(),
                    content: json,
                    usage: Default::default(),
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
                unimplemented!()
            }

            async fn embed(
                &self,
                _texts: &[String],
            ) -> Result<Vec<intelligence_core::types::Embedding>, intelligence_core::ModelError>
            {
                unimplemented!()
            }

            async fn health(&self) -> Result<(), intelligence_core::ModelError> {
                Ok(())
            }

            async fn pipeline_stats(
                &self,
            ) -> Result<intelligence_core::types::IntelligenceStats, intelligence_core::ModelError>
            {
                unimplemented!()
            }

            async fn conversation(
                &self,
                _id: &intelligence_core::types::ConversationId,
            ) -> Result<Vec<intelligence_core::types::ModelResponse>, intelligence_core::ModelError>
            {
                unimplemented!()
            }
        }

        Planner::new(Arc::new(MockPlannerCoordinator(result)))
    }

    #[tokio::test]
    async fn test_planner_returns_plan() {
        let plan = Plan {
            actions: vec![PlannedAction {
                action_type: "respond".into(),
                topics: vec![],
                reason: "user said hello".into(),
            }],
        };
        let planner = make_planner(plan);
        let result = planner.plan("hello", "", "", false).await;
        assert_eq!(result.actions.len(), 1);
        assert_eq!(result.actions[0].action_type, "respond");
    }

    #[tokio::test]
    async fn test_planner_empty_plan_falls_back() {
        let plan = Plan { actions: vec![] };
        let planner = make_planner(plan);
        let result = planner.plan("hello", "", "", false).await;
        assert_eq!(result.actions.len(), 1);
        assert_eq!(result.actions[0].action_type, "respond");
    }
}
