//! A fake intelligence coordinator that returns scripted plans.

use std::sync::Arc;

use async_trait::async_trait;
use intelligence_core::traits::IntelligenceCoordinator;
use intelligence_core::types::{
    ConversationId, Embedding, IntelligenceStats, ModelInput, ModelRequest, ModelResponse,
    StreamChunk,
};
use intelligence_core::{ModelError, ModelResult};

/// Returns the planner plan when the prompt is a planning prompt, and a
/// canned "reply" otherwise.
#[derive(Debug)]
pub struct FakeCoordinator {
    plan_json: &'static str,
    reply: &'static str,
}

impl FakeCoordinator {
    /// A coordinator that always plans the given JSON.
    pub fn plan(plan_json: &'static str) -> Arc<Self> {
        Arc::new(Self {
            plan_json,
            reply: "Done.",
        })
    }

    /// Set the canned reply returned for non-planning prompts.
    pub fn with_reply(mut self: Arc<Self>, reply: &'static str) -> Arc<Self> {
        let inner = Arc::get_mut(&mut self).expect("no aliasing while building");
        inner.reply = reply;
        self
    }

    /// The canned reply (the simulated "Telegram reply").
    pub fn reply(&self) -> &'static str {
        self.reply
    }

    /// Simulate the reply generation step of the Cognitive Loop.
    pub async fn simulate_reply(&self) -> String {
        let request = ModelRequest {
            request_id: intelligence_core::types::RequestId::new(),
            capability: intelligence_core::types::CapabilityKind::Chat,
            model_id: None,
            input: ModelInput::Text("compose the reply".into()),
            parameters: Default::default(),
            temperature: None,
            top_p: None,
            max_tokens: None,
            stream: false,
        };
        self.request(request).await.unwrap().content
    }
}

#[async_trait]
impl IntelligenceCoordinator for FakeCoordinator {
    async fn request(&self, request: ModelRequest) -> ModelResult<ModelResponse> {
        let input_text = match &request.input {
            ModelInput::Text(t) => t.as_str(),
            _ => "",
        };
        let content = if input_text.contains("planning engine") {
            self.plan_json
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
            provider: "fake".into(),
            detail: "streaming not supported".into(),
        })
    }

    async fn embed(&self, _texts: &[String]) -> ModelResult<Vec<Embedding>> {
        Err(ModelError::InvalidResponse {
            provider: "fake".into(),
            detail: "embedding not supported".into(),
        })
    }

    async fn conversation(
        &self,
        _conversation: &ConversationId,
    ) -> ModelResult<Vec<ModelResponse>> {
        Err(ModelError::InvalidResponse {
            provider: "fake".into(),
            detail: "conversation history not supported".into(),
        })
    }

    async fn health(&self) -> ModelResult<()> {
        Ok(())
    }

    async fn pipeline_stats(&self) -> ModelResult<IntelligenceStats> {
        Err(ModelError::InvalidResponse {
            provider: "fake".into(),
            detail: "stats not supported".into(),
        })
    }
}
