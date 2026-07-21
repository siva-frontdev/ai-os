use super::error::*;
use async_trait::async_trait;
use intelligence_core::error::ModelResult;
use intelligence_core::traits::{ContextAssembler, PromptRenderer};
use intelligence_core::types::{
    ContextWindow, ConversationId, ModelInput, ModelRequest, TruncationStrategy,
};
use std::sync::Arc;
use tokio::sync::RwLock;

fn input_text(input: &ModelInput) -> String {
    match input {
        ModelInput::Text(s) => s.clone(),
        ModelInput::Multimodal(parts) => parts
            .iter()
            .map(|p| String::from_utf8_lossy(&p.data).into_owned())
            .collect(),
    }
}

#[derive(Debug, Default)]
pub struct DefaultContextBuilder {
    prompt_manager: Option<Arc<dyn PromptRenderer>>,
    truncation_strategy: RwLock<Option<TruncationStrategy>>,
}

impl DefaultContextBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_prompt_manager(mut self, pm: Arc<dyn PromptRenderer>) -> Self {
        self.prompt_manager = Some(pm);
        self
    }

    async fn assemble_internal(
        &self,
        request: &ModelRequest,
        _conversation: Option<&ConversationId>,
    ) -> ModelResult<ContextWindow> {
        let text = input_text(&request.input);
        let total_tokens = text.split_whitespace().count() as u32;
        Ok(ContextWindow {
            system_prompt: None,
            messages: vec![text],
            retrieved_memories: vec![],
            observations: vec![],
            tool_definitions: vec![],
            total_tokens,
            max_tokens: 8192,
            truncated: false,
        })
    }
}

#[async_trait]
impl ContextAssembler for DefaultContextBuilder {
    async fn assemble(
        &self,
        request: &ModelRequest,
        conversation: Option<&ConversationId>,
    ) -> ModelResult<ContextWindow> {
        self.assemble_internal(request, conversation).await
    }

    async fn assemble_with_memory(
        &self,
        request: &ModelRequest,
        conversation: Option<&ConversationId>,
        _memory_query: &str,
    ) -> ModelResult<ContextWindow> {
        self.assemble_internal(request, conversation).await
    }

    fn set_truncation_strategy(&self, strategy: TruncationStrategy) {
        let mut guard = self.truncation_strategy.blocking_write();
        *guard = Some(strategy);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use intelligence_core::types::{CapabilityKind, ModelId, ModelInput, RequestId};

    #[tokio::test]
    async fn assembles_window() {
        let builder = DefaultContextBuilder::new();
        let req = ModelRequest {
            request_id: RequestId::new(),
            capability: CapabilityKind::Chat,
            model_id: None,
            input: ModelInput::Text("hello world".into()),
            parameters: Default::default(),
            temperature: None,
            top_p: None,
            max_tokens: None,
            stream: false,
        };
        let ctx = builder.assemble(&req, None).await.unwrap();
        assert_eq!(ctx.messages.len(), 1);
    }

    #[tokio::test]
    async fn assemble_returns_model_result() {
        let builder = DefaultContextBuilder::new();
        let req = ModelRequest {
            request_id: RequestId::new(),
            capability: CapabilityKind::Chat,
            model_id: Some(ModelId::new()),
            input: ModelInput::Text("test".into()),
            parameters: Default::default(),
            temperature: Some(0.5),
            top_p: None,
            max_tokens: Some(16),
            stream: false,
        };
        let result: ModelResult<ContextWindow> = builder.assemble(&req, None).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn implement_context_assembler_trait() {
        let builder: Arc<dyn ContextAssembler> = Arc::new(DefaultContextBuilder::new());
        let req = ModelRequest {
            request_id: RequestId::new(),
            capability: CapabilityKind::Chat,
            model_id: None,
            input: ModelInput::Text("trait impl".into()),
            parameters: Default::default(),
            temperature: None,
            top_p: None,
            max_tokens: None,
            stream: false,
        };
        let ctx = builder.assemble(&req, None).await.unwrap();
        assert_eq!(ctx.messages, vec!["trait impl"]);
    }

    #[tokio::test]
    async fn set_truncation_strategy_does_not_panic() {
        let builder = DefaultContextBuilder::new();
        builder.set_truncation_strategy(TruncationStrategy::SlidingWindow { keep_last_n: 100 });
    }
}
