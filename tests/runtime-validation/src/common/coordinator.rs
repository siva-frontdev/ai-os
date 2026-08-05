//! A scripted intelligence coordinator for the cognitive-loop scenarios.

use std::sync::Arc;

use async_trait::async_trait;
use brain_coordinator::cognitive_loop::MockCoordinator;
use intelligence_coordinator::world_understanding::{
    StructuredWorldUpdate, WorldUnderstandingService,
};
use intelligence_core::traits::IntelligenceCoordinator;
use intelligence_core::types::{
    ConversationId, Embedding, IntelligenceStats, ModelInput, ModelRequest, ModelResponse,
    StreamChunk,
};
use intelligence_core::{ModelError, ModelResult};

/// Returns predetermined JSON per call site, identified by keywords in the
/// prompt — the same contract the real planner/memory-evaluator/executor
/// consume.
///
/// - prompt contains `planning engine`  → `plan_json` (Planner)
/// - prompt contains `memory evaluator` → `memory_json` (MemoryEvaluator)
/// - anything else                      → `reply` (final response)
#[derive(Debug)]
pub struct ScriptedCoordinator {
    plan_json: &'static str,
    memory_json: &'static str,
    reply: &'static str,
}

impl ScriptedCoordinator {
    /// A coordinator that plans `plan_json`, stores everything per
    /// `memory_json`, and replies with `reply`.
    pub fn new(
        plan_json: &'static str,
        memory_json: &'static str,
        reply: &'static str,
    ) -> Arc<Self> {
        Arc::new(Self {
            plan_json,
            memory_json,
            reply,
        })
    }
}

#[async_trait]
impl IntelligenceCoordinator for ScriptedCoordinator {
    async fn request(&self, request: ModelRequest) -> ModelResult<ModelResponse> {
        let input = match &request.input {
            ModelInput::Text(t) => t.as_str(),
            _ => "",
        };
        let content = if input.contains("planning engine") || input.contains("plan what actions") {
            self.plan_json
        } else if input.contains("memory evaluator") {
            self.memory_json
        } else {
            self.reply
        };
        Ok(ModelResponse {
            request_id: intelligence_core::types::RequestId::new(),
            model_id: intelligence_core::types::ModelId::new(),
            content: content.to_string(),
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
    ) -> ModelResult<Box<dyn futures::Stream<Item = StreamChunk> + Send>> {
        Err(ModelError::InvalidResponse {
            provider: "scripted".into(),
            detail: "streaming not supported".into(),
        })
    }

    async fn embed(&self, _texts: &[String]) -> ModelResult<Vec<Embedding>> {
        Err(ModelError::InvalidResponse {
            provider: "scripted".into(),
            detail: "embedding not supported".into(),
        })
    }

    async fn conversation(
        &self,
        _conversation: &ConversationId,
    ) -> ModelResult<Vec<ModelResponse>> {
        Err(ModelError::InvalidResponse {
            provider: "scripted".into(),
            detail: "conversation history not supported".into(),
        })
    }

    async fn health(&self) -> ModelResult<()> {
        Ok(())
    }

    async fn pipeline_stats(&self) -> ModelResult<IntelligenceStats> {
        Err(ModelError::InvalidResponse {
            provider: "scripted".into(),
            detail: "stats not supported".into(),
        })
    }
}

/// A [`WorldUnderstandingService`] that returns the given update for every
/// observation, via the crate's reusable mock coordinator.
pub fn scripted_understanding(update: StructuredWorldUpdate) -> WorldUnderstandingService {
    WorldUnderstandingService::new(Arc::new(MockCoordinator(update)))
}
