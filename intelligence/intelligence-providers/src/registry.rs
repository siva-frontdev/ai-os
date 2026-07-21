use super::error::*;
use async_trait::async_trait;
use intelligence_core::error::ModelResult;
use intelligence_core::traits::ModelProvider;
use intelligence_core::types::{ProviderHealth, ProviderId};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};

#[derive(Debug, Default)]
pub struct DefaultProviderRegistry {
    providers: RwLock<HashMap<ProviderId, Arc<dyn intelligence_core::traits::ModelProvider>>>,
    health: RwLock<HashMap<ProviderId, ProviderHealth>>,
}

impl DefaultProviderRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn register(
        &self,
        id: ProviderId,
        provider: Arc<dyn intelligence_core::traits::ModelProvider>,
    ) -> ProviderResult<()> {
        self.providers.write().await.insert(id, provider);
        Ok(())
    }

    pub async fn deregister(&self, id: &ProviderId) -> ProviderResult<()> {
        self.providers.write().await.remove(id);
        self.health.write().await.remove(id);
        Ok(())
    }

    pub async fn get(
        &self,
        id: &ProviderId,
    ) -> ProviderResult<Arc<dyn intelligence_core::traits::ModelProvider>> {
        self.providers
            .read()
            .await
            .get(id)
            .cloned()
            .ok_or_else(|| ProviderError::NotFound(id.to_string()))
    }

    pub async fn list_ids(&self) -> Vec<ProviderId> {
        self.providers.read().await.keys().cloned().collect()
    }

    pub async fn update_health(&self, id: ProviderId, health: ProviderHealth) {
        self.health.write().await.insert(id, health);
    }

    pub async fn run_health_checks(&self) {
        let ids: Vec<ProviderId> = self.providers.read().await.keys().cloned().collect();
        let mut health = self.health.write().await;
        for id in ids {
            let providers = self.providers.read().await;
            if let Some(p) = providers.get(&id) {
                match p.health_check().await {
                    Ok(_) => {
                        health.insert(id, ProviderHealth::Healthy);
                    }
                    Err(_) => {
                        health.insert(
                            id,
                            ProviderHealth::Unhealthy {
                                reason: "health check failed".into(),
                            },
                        );
                    }
                }
            }
        }
    }

    pub fn start_health_task(self: Arc<Self>, period: Duration) {
        tokio::spawn(async move {
            let mut ticker = interval(period);
            loop {
                ticker.tick().await;
                self.run_health_checks().await;
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[derive(Debug)]
    struct FakeProvider {
        id: ProviderId,
    }

    #[async_trait]
    impl intelligence_core::traits::ModelProvider for FakeProvider {
        async fn chat(
            &self,
            _: &intelligence_core::types::ModelRequest,
        ) -> intelligence_core::error::ModelResult<intelligence_core::types::ModelResponse>
        {
            Ok(intelligence_core::types::ModelResponse {
                request_id: intelligence_core::types::RequestId::new(),
                model_id: intelligence_core::types::ModelId::new(),
                content: String::new(),
                usage: Default::default(),
                finished: true,
                finish_reason: None,
            })
        }
        async fn chat_stream(
            &self,
            _: &intelligence_core::types::ModelRequest,
        ) -> intelligence_core::error::ModelResult<intelligence_core::traits::ChatStream> {
            Ok(futures::stream::empty())
        }
        async fn embed(
            &self,
            _: &intelligence_core::types::ModelRequest,
        ) -> intelligence_core::error::ModelResult<Vec<intelligence_core::types::Embedding>>
        {
            Ok(vec![])
        }
        async fn classify(
            &self,
            _: &intelligence_core::types::ModelRequest,
        ) -> intelligence_core::error::ModelResult<String> {
            Ok(String::new())
        }
        async fn health_check(
            &self,
        ) -> intelligence_core::error::ModelResult<intelligence_core::types::ModelInfo> {
            Ok(intelligence_core::types::ModelInfo {
                model_id: intelligence_core::types::ModelId::new(),
                provider_id: self.id,
                name: "fake".into(),
                version: "0".into(),
                capabilities: vec![],
                pricing: intelligence_core::types::ModelPricing {
                    currency: "USD".into(),
                    input_per_million_tokens: 0.0,
                    output_per_million_tokens: 0.0,
                    minimum_charge: None,
                    free_tier_tokens: None,
                },
                latency_p50_ms: 100,
                latency_p99_ms: 500,
                availability: intelligence_core::types::AvailabilityStatus::Available,
                tags: vec![],
                max_batch_size: None,
            })
        }
        fn capabilities(&self) -> Vec<intelligence_core::types::ModelCapability> {
            vec![]
        }
        fn provider_id(&self) -> ProviderId {
            self.id
        }
    }

    #[tokio::test]
    async fn register_and_remove() {
        let reg = DefaultProviderRegistry::new();
        let id = ProviderId::new();
        let p: Arc<dyn intelligence_core::traits::ModelProvider> = Arc::new(FakeProvider { id });
        reg.register(id, p.clone()).await.unwrap();
        reg.get(&id).await.unwrap();
        reg.deregister(&id).await.unwrap();
        assert!(reg.get(&id).await.is_err());
    }
}
