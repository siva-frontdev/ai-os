use super::error::*;
use async_trait::async_trait;
use intelligence_core::error::{ModelError, ModelResult};
use intelligence_core::traits::EmbeddingGenerator;
use intelligence_core::types::{
    Embedding, EmbeddingId, ModelId, ModelRequest, ProviderId, RequestId,
};
use std::sync::Arc;

static EMBEDDING_MODEL_ID: std::sync::OnceLock<ModelId> = std::sync::OnceLock::new();

fn model_id() -> ModelId {
    *EMBEDDING_MODEL_ID.get_or_init(|| ModelId::new())
}

#[derive(Debug)]
pub struct NoOpProvider;

#[async_trait]
impl intelligence_core::traits::ModelProvider for NoOpProvider {
    fn capabilities(&self) -> Vec<intelligence_core::types::ModelCapability> {
        vec![]
    }
    fn provider_id(&self) -> ProviderId {
        ProviderId::new()
    }
    async fn chat(
        &self,
        _request: &ModelRequest,
    ) -> ModelResult<intelligence_core::types::ModelResponse> {
        Ok(intelligence_core::types::ModelResponse {
            request_id: RequestId::new(),
            model_id: ModelId::new(),
            content: String::new(),
            usage: Default::default(),
            finished: true,
            finish_reason: None,
        })
    }
    async fn chat_stream(
        &self,
        _request: &ModelRequest,
    ) -> ModelResult<intelligence_core::traits::ChatStream> {
        use futures::stream;
        Ok(stream::empty())
    }
    async fn embed(&self, _request: &ModelRequest) -> ModelResult<Vec<Embedding>> {
        Ok(vec![])
    }
    async fn classify(&self, _request: &ModelRequest) -> ModelResult<String> {
        Ok(String::new())
    }
    async fn health_check(&self) -> ModelResult<intelligence_core::types::ModelInfo> {
        unimplemented!()
    }
}

#[derive(Debug, Clone)]
pub struct DefaultEmbeddingManager {
    provider: Arc<dyn intelligence_core::traits::ModelProvider>,
}

impl DefaultEmbeddingManager {
    pub fn new(provider: Arc<dyn intelligence_core::traits::ModelProvider>) -> Self {
        Self { provider }
    }

    pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        let (mut dot, mut norm_a, mut norm_b) = (0.0f32, 0.0f32, 0.0f32);
        for i in 0..a.len().min(b.len()) {
            dot += a[i] * b[i];
            norm_a += a[i] * a[i];
            norm_b += b[i] * b[i];
        }
        let denom = (norm_a * norm_b).sqrt();
        if denom == 0.0 {
            0.0
        } else {
            dot / denom
        }
    }
}

impl Default for DefaultEmbeddingManager {
    fn default() -> Self {
        Self {
            provider: Arc::new(NoOpProvider),
        }
    }
}

#[async_trait]
impl EmbeddingGenerator for DefaultEmbeddingManager {
    async fn generate(&self, texts: &[String]) -> ModelResult<Vec<Embedding>> {
        if texts.is_empty() {
            return Ok(vec![]);
        }
        let request = ModelRequest {
            request_id: RequestId::new(),
            capability: intelligence_core::types::CapabilityKind::Embedding,
            model_id: None,
            input: intelligence_core::types::ModelInput::Text(texts.join("\n")),
            parameters: Default::default(),
            temperature: None,
            top_p: None,
            max_tokens: None,
            stream: false,
        };
        self.provider.embed(&request).await
    }

    async fn generate_single(&self, text: &str) -> ModelResult<Embedding> {
        let results = self.generate(&[text.to_string()]).await?;
        results
            .into_iter()
            .next()
            .ok_or_else(|| ModelError::InvalidResponse {
                provider: "embedding".into(),
                detail: "empty result".into(),
            })
    }

    async fn similarity(&self, a: &Embedding, b: &Embedding) -> ModelResult<f32> {
        Ok(Self::cosine_similarity(&a.vector, &b.vector))
    }

    async fn search(
        &self,
        query: &Embedding,
        corpus: &[Embedding],
        top_k: u32,
    ) -> ModelResult<Vec<(usize, f32)>> {
        let mut scored: Vec<(usize, f32)> = corpus
            .iter()
            .enumerate()
            .map(|(i, emb)| (i, Self::cosine_similarity(&query.vector, &emb.vector)))
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(top_k as usize);
        Ok(scored)
    }

    fn dimensions(&self) -> u32 {
        384
    }

    fn model_id(&self) -> ModelId {
        model_id()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use intelligence_core::types::{ModelId, RequestId};

    #[test]
    fn cosine_identical() {
        let v = vec![1.0, 0.0, 0.0];
        assert!((DefaultEmbeddingManager::cosine_similarity(&v, &v) - 1.0).abs() < 1e-4);
    }

    #[test]
    fn cosine_orthogonal() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        assert!((DefaultEmbeddingManager::cosine_similarity(&a, &b) - 0.0).abs() < 1e-4);
    }

    #[tokio::test]
    async fn generate_returns_empty_with_noop() {
        let mgr = DefaultEmbeddingManager::default();
        let result = mgr.generate(&["hello".into()]).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn search_returns_sorted() {
        let mgr = DefaultEmbeddingManager::default();
        let q = Embedding {
            id: EmbeddingId::new(),
            vector: vec![1.0, 0.0],
        };
        let corpus = vec![
            Embedding {
                id: EmbeddingId::new(),
                vector: vec![0.0, 1.0],
            },
            Embedding {
                id: EmbeddingId::new(),
                vector: vec![1.0, 0.0],
            },
        ];
        let result = mgr.search(&q, &corpus, 2).await.unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].0, 1);
    }

    #[test]
    fn dimensions_returns_384() {
        let mgr = DefaultEmbeddingManager::default();
        assert_eq!(mgr.dimensions(), 384);
    }

    #[test]
    fn model_id_is_static() {
        let mgr = DefaultEmbeddingManager::default();
        let a = mgr.model_id();
        let b = mgr.model_id();
        assert_eq!(a, b);
    }
}
