//! # Brain Model
//!
//! Provider-neutral AI model abstraction layer.
//!
//! The Brain Platform never imports a specific model provider SDK
//! (OpenAI, Anthropic, Ollama, etc.). All model interactions go through
//! the traits in this crate.
//!
//! ## Modules
//!
//! | Module | Responsibility |
//! |---|---|
//! | [`provider`] | `ModelProvider` — registration, health, completion |
//! | [`reasoning`] | `ReasoningModel` — chain-of-thought, hypothesis generation |
//! | [`planning`] | `PlanningModel` — task graphs, plan alternatives |
//! | [`embedding`] | `EmbeddingProvider` — vector embeddings |
//! | [`prompt`] | `PromptRenderer` — template rendering |
//! | [`response`] | `ResponseParser` — parse raw outputs |
//! | [`context`] | `ConversationContext` — message history |
//! | [`registry`] | `ModelProviderRegistry` — route to providers |
//! | [`mock`] | `MockModelProvider` — test double |
//!
//! ## Design decisions
//!
//! * **No provider SDK imports anywhere in this crate.**
//! * **Registry-lookup pattern**: the Coordinator holds a `&dyn ModelProviderRegistry`
//!   and uses it to resolve provider names to trait objects.
//! * **Circuit breaker contract**: providers may be temporarily unavailable after
//!   repeated failures. The registry marks them `CircuitBreakerOpen` and
//!   routes to the `fallback_provider` instead.

pub mod context;
pub mod embedding;
pub mod mock;
pub mod planning;
pub mod prompt;
pub mod provider;
pub mod reasoning;
pub mod registry;
pub mod response;

pub use provider::{
    Completion, ConversationContext, Embedding, EmbeddingProvider, FinishReason,
    Message, MessageRole, ModelConfig, ModelId, ModelProvider, ModelProviderError,
    PlanningModel, PromptRenderer, ProviderCapabilities, ProviderStatus,
    ProviderStatus::CircuitBreakerOpen,
    ProviderStatus::CircuitBreakerClosed,
    ReasoningModel, ResponseParser, TokenCount, TokenCounter,
};
pub use registry::ModelProviderRegistry;
pub use mock::MockModelProvider;
