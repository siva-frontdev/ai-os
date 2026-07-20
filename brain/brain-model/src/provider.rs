//! `ModelProvider` — primary trait for model providers.

pub use brain_core::context::{PlanningContext, ReasoningContext};
use brain_core::errors::BrainError;
use memory_core::Timestamp;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Debug;

pub type ModelProviderError = BrainError;

/// Opaque model identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ModelId(pub uuid::Uuid);

impl ModelId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
    pub const fn from_uuid(uuid: uuid::Uuid) -> Self {
        Self(uuid)
    }
    pub const fn as_uuid(&self) -> &uuid::Uuid {
        &self.0
    }
}
impl Default for ModelId {
    fn default() -> Self {
        Self::new()
    }
}

/// Configuration for a specific model call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelConfig {
    pub provider: String,
    pub model_name: String,
    pub parameters: HashMap<String, String>,
    pub timeout_ms: Option<u64>,
}

impl ModelConfig {
    pub fn new(provider: impl Into<String>, model_name: impl Into<String>) -> Self {
        Self {
            provider: provider.into(),
            model_name: model_name.into(),
            parameters: HashMap::new(),
            timeout_ms: Some(30000),
        }
    }
}

/// Capabilities advertised by a provider.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderCapabilities {
    pub supports_reasoning: bool,
    pub supports_planning: bool,
    pub supports_embedding: bool,
    pub max_context_tokens: u32,
    pub supports_streaming: bool,
}

impl Default for ProviderCapabilities {
    fn default() -> Self {
        Self {
            supports_reasoning: false,
            supports_planning: false,
            supports_embedding: false,
            max_context_tokens: 4096,
            supports_streaming: false,
        }
    }
}

/// Runtime health status of a provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProviderStatus {
    Healthy,
    Degraded,
    Unavailable,
    CircuitBreakerOpen,
    CircuitBreakerClosed,
}

/// Role of a message in a conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

/// A single message in a prompt.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
    pub name: Option<String>,
}

impl Message {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::System,
            content: content.into(),
            name: None,
        }
    }
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::User,
            content: content.into(),
            name: None,
        }
    }
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: content.into(),
            name: None,
        }
    }
}

/// Completion response from a model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Completion {
    pub id: ModelId,
    pub model: String,
    pub content: String,
    pub usage: TokenCount,
    pub finish_reason: FinishReason,
    pub created_at: Timestamp,
}

/// Why a completion ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FinishReason {
    Stop,
    Length,
    ContentFilter,
    ToolCalls,
    Error,
}

/// Token usage in a single completion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TokenCount {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Vector embedding result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Embedding {
    pub model: String,
    pub values: Vec<f32>,
    pub dimensions: usize,
}

/// Primary model provider trait.
#[async_trait::async_trait]
pub trait ModelProvider: Debug + Send + Sync {
    fn id(&self) -> ModelId;
    fn name(&self) -> &str;
    async fn complete(
        &self,
        messages: &[Message],
        config: &ModelConfig,
    ) -> Result<Completion, ModelProviderError>;
    async fn embed(&self, text: &str) -> Result<Embedding, ModelProviderError>;
    fn status(&self) -> ProviderStatus;
    async fn health_check(&self) -> Result<(), ModelProviderError>;
}

/// Model capable of chain-of-thought reasoning.
#[async_trait::async_trait]
pub trait ReasoningModel: Send + Sync {
    async fn reason(
        &self,
        ctx: &ReasoningContext,
        config: &ModelConfig,
    ) -> Result<Completion, ModelProviderError>;
    async fn generate_hypotheses(
        &self,
        observations: &[String],
        config: &ModelConfig,
    ) -> Result<Vec<String>, ModelProviderError>;
}

/// Model capable of generating plans.
#[async_trait::async_trait]
pub trait PlanningModel: Send + Sync {
    async fn generate_plan(
        &self,
        goal: &str,
        context: &PlanningContext,
        config: &ModelConfig,
    ) -> Result<String, ModelProviderError>;
}

/// Embedding provider.
#[async_trait::async_trait]
pub trait EmbeddingProvider: Send + Sync {
    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Embedding>, ModelProviderError>;
    fn dimensions(&self) -> usize;
}

/// Prompt template renderer.
#[async_trait::async_trait]
pub trait PromptRenderer: Send + Sync {
    fn render_system_prompt(
        &self,
        template: &str,
        vars: &HashMap<String, String>,
    ) -> Result<String, ModelProviderError>;
    fn render_messages(&self, messages: &[Message]) -> Result<String, ModelProviderError>;
}

/// Parses raw model text output into typed completions.
#[async_trait::async_trait]
pub trait ResponseParser: Send + Sync {
    fn parse_completion(&self, raw: &str) -> Result<Completion, ModelProviderError>;
}

/// Manages conversation history.
#[async_trait::async_trait]
pub trait ConversationContext: Send + Sync {
    fn push(&mut self, message: Message);
    fn history(&self) -> Vec<Message>;
    fn clear(&mut self);
}

/// Counts tokens without a full model call.
#[async_trait::async_trait]
pub trait TokenCounter: Send + Sync {
    fn count_tokens(&self, text: &str) -> TokenCount;
}
