//! `MockModelProvider` — controllable test double that returns predetermined responses.
use super::*;
use async_trait::async_trait;
use brain_core::context::{ReasoningContext, PlanningContext};
use brain_core::errors::BrainError;
use memory_core::Timestamp;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone)]
pub struct MockModelProvider {
    responses: Arc<RwLock<VecDeque<Completion>>>,
    embeddings: Arc<RwLock<VecDeque<Vec<f32>>>>,
    fail_next_call: Arc<RwLock<bool>>,
    call_count: Arc<AtomicU64>,
    health_status: Arc<RwLock<ProviderStatus>>,
}

impl MockModelProvider {
    pub fn new() -> Self {
        Self {
            responses: Arc::new(RwLock::new(VecDeque::new())),
            embeddings: Arc::new(RwLock::new(VecDeque::new())),
            fail_next_call: Arc::new(RwLock::new(false)),
            call_count: Arc::new(AtomicU64::new(0)),
            health_status: Arc::new(RwLock::new(ProviderStatus::Healthy)),
        }
    }

    /// Queue a completion response to return on next call.
    pub fn enqueue_completion(&self, completion: Completion) {
        self.responses.write().unwrap().push_back(completion);
    }

    /// Queue a simple text completion response.
    pub fn enqueue_text(&self, text: impl Into<String>, tokens: u32) {
        let completion = Completion {
            id: ModelId::new(),
            model: "mock-model".into(),
            content: text.into(),
            usage: TokenCount { prompt_tokens: 10, completion_tokens: tokens, total_tokens: 10 + tokens },
            finish_reason: FinishReason::Stop,
            created_at: Timestamp::now(),
        };
        self.enqueue_completion(completion);
    }

    /// Queue an embedding response.
    pub fn enqueue_embedding(&self, embedding: Vec<f32>) {
        self.embeddings.write().unwrap().push_back(embedding);
    }

    /// Make the next call fail.
    pub fn set_fail_next(&self) {
        *self.fail_next_call.write().unwrap() = true;
    }

    /// Set the provider health status.
    pub fn set_health_status(&self, status: ProviderStatus) {
        *self.health_status.write().unwrap() = status;
    }

    /// Return how many calls have been made.
    pub fn call_count(&self) -> u64 {
        self.call_count.load(Ordering::Relaxed)
    }
}

#[async_trait]
impl ModelProvider for MockModelProvider {
    fn id(&self) -> ModelId { ModelId::new() }
    fn name(&self) -> &str { "mock-model" }

    async fn complete(&self, _messages: &[Message], _config: &ModelConfig)
        -> Result<Completion, ModelProviderError>
    {
        self.call_count.fetch_add(1, Ordering::Relaxed);

        if *self.fail_next_call.read().unwrap() {
            *self.fail_next_call.write().unwrap() = false;
            return Err(BrainError::ModelProviderError("mock failure".into()).into());
        }

        self.responses
            .write().unwrap()
            .pop_front()
            .ok_or_else(|| BrainError::ModelProviderError("no more queued responses".into()).into())
    }

    async fn embed(&self, _text: &str) -> Result<Embedding, ModelProviderError> {
        self.call_count.fetch_add(1, Ordering::Relaxed);

        self.embeddings
            .write().unwrap()
            .pop_front()
            .ok_or_else(|| BrainError::ModelProviderError("no more queued embeddings".into()).into())
            .map(|values| { let dim = values.len(); Embedding { model: "mock-embed".into(), values, dimensions: dim } })
    }

    fn status(&self) -> ProviderStatus {
        self.health_status.read().unwrap().clone()
    }

    async fn health_check(&self) -> Result<(), ModelProviderError> {
        match self.status() {
            ProviderStatus::Healthy => Ok(()),
            ProviderStatus::Unavailable => Err(BrainError::ModelProviderError("unavailable".into()).into()),
            _ => Ok(()),
        }
    }
}

#[async_trait]
impl ReasoningModel for MockModelProvider {
    async fn reason(&self, _ctx: &ReasoningContext, _config: &ModelConfig)
        -> Result<Completion, ModelProviderError>
    {
        self.complete(&[], _config).await
    }

    async fn generate_hypotheses(&self, _obs: &[String], _config: &ModelConfig)
        -> Result<Vec<String>, ModelProviderError>
    {
        Ok(vec!["mock-hypothesis".into()])
    }
}

#[async_trait]
impl PlanningModel for MockModelProvider {
    async fn generate_plan(&self, _goal: &str, _ctx: &PlanningContext, _config: &ModelConfig)
        -> Result<String, ModelProviderError>
    {
        Ok("mock-plan".into())
    }
}

#[async_trait]
impl EmbeddingProvider for MockModelProvider {
    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Embedding>, ModelProviderError> {
        let mut out = Vec::with_capacity(texts.len());
        for _ in texts {
            out.push(self.embed("").await?);
        }
        Ok(out)
    }

    fn dimensions(&self) -> usize { 768 }
}

#[async_trait]
impl PromptRenderer for MockModelProvider {
    fn render_system_prompt(&self, template: &str, vars: &HashMap<String, String>)
        -> Result<String, ModelProviderError>
    {
        let mut out = template.to_string();
        for (k, v) in vars {
            out = out.replace(&format!("{{{{{}}}}}", k), v);
        }
        Ok(out)
    }

    fn render_messages(&self, messages: &[Message]) -> Result<String, ModelProviderError> {
        Ok(messages.iter().map(|m| format!("{:?}: {}", m.role, m.content)).collect())
    }
}

#[async_trait]
impl ResponseParser for MockModelProvider {
    fn parse_completion(&self, raw: &str) -> Result<Completion, ModelProviderError> {
        Ok(Completion {
            id: ModelId::new(),
            model: "mock-model".into(),
            content: raw.to_string(),
            usage: TokenCount { prompt_tokens: 10, completion_tokens: (raw.len() / 4) as u32, total_tokens: 20 },
            finish_reason: FinishReason::Stop,
            created_at: Timestamp::now(),
        })
    }
}

impl ConversationContext for MockModelProvider {
    fn push(&mut self, _message: Message) {}
    fn history(&self) -> Vec<Message> { Vec::new() }
    fn clear(&mut self) {}
}

impl TokenCounter for MockModelProvider {
    fn count_tokens(&self, text: &str) -> TokenCount {
        let total = (text.len() / 4) as u32;
        TokenCount { prompt_tokens: total, completion_tokens: 0, total_tokens: total }
    }
}
