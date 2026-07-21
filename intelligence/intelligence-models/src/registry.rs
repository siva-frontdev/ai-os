use super::error::*;
use async_trait::async_trait;
use intelligence_core::error::ModelError;
use intelligence_core::traits::ModelRegistry;
use intelligence_core::types::{
    AvailabilityStatus, CapabilityKind, ModelId, ModelInfo, ModelQuery,
};
use std::collections::{HashMap, HashSet};
use tokio::sync::RwLock;

fn to_model_result<T>(res: ModelsResult<T>) -> intelligence_core::error::ModelResult<T> {
    res.map_err(|e| ModelError::Io(e.to_string()))
}

#[derive(Debug, Default)]
pub struct DefaultModelRegistry {
    by_id: RwLock<HashMap<ModelId, ModelInfo>>,
    by_capability: RwLock<HashMap<CapabilityKind, HashSet<ModelId>>>,
}

impl DefaultModelRegistry {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl ModelRegistry for DefaultModelRegistry {
    async fn register_model(&self, info: ModelInfo) -> intelligence_core::error::ModelResult<()> {
        if self.by_id.read().await.contains_key(&info.model_id) {
            return Err(ModelError::Io(format!(
                "model {} already registered",
                info.model_id
            )));
        }
        let caps: Vec<CapabilityKind> = info.capabilities.iter().map(|c| c.kind.clone()).collect();
        self.by_id.write().await.insert(info.model_id, info.clone());
        if !caps.is_empty() {
            let mut by_cap = self.by_capability.write().await;
            for cap in &caps {
                by_cap.entry(cap.clone()).or_default().insert(info.model_id);
            }
        }
        Ok(())
    }
    async fn deregister_model(
        &self,
        model_id: &ModelId,
    ) -> intelligence_core::error::ModelResult<()> {
        let Some(info) = self.by_id.write().await.remove(model_id) else {
            return Ok(());
        };
        let caps: Vec<CapabilityKind> = info.capabilities.iter().map(|c| c.kind.clone()).collect();
        if !caps.is_empty() {
            let mut by_cap = self.by_capability.write().await;
            for cap in &caps {
                if let Some(ids) = by_cap.get_mut(cap) {
                    ids.remove(model_id);
                    if ids.is_empty() {
                        by_cap.remove(cap);
                    }
                }
            }
        }
        Ok(())
    }
    async fn query(
        &self,
        query: &ModelQuery,
    ) -> intelligence_core::error::ModelResult<Vec<ModelInfo>> {
        let by_id = self.by_id.read().await;
        let mut results: Vec<ModelInfo> = by_id.values().cloned().collect();
        if !query.required_capabilities.is_empty() {
            let by_cap = self.by_capability.read().await;
            let mut valid = HashSet::new();
            for cap in &query.required_capabilities {
                if let Some(ids) = by_cap.get(cap) {
                    valid.extend(ids.iter().cloned());
                }
            }
            results.retain(|m| valid.contains(&m.model_id));
        }
        results.retain(|m| {
            if let AvailabilityStatus::Unavailable { .. } = m.availability {
                return false;
            }
            if let Some(max_lat) = query.max_latency_ms {
                if m.latency_p99_ms > max_lat {
                    return false;
                }
            }
            if let Some(max_cost) = query.max_cost_cents {
                if m.pricing.output_per_million_tokens > max_cost {
                    return false;
                }
            }
            true
        });
        if let Some(ref tags) = query.tags {
            if !tags.is_empty() {
                results.retain(|m| tags.iter().all(|t| m.tags.contains(t)));
            }
        }
        Ok(results)
    }
    async fn get(
        &self,
        model_id: &ModelId,
    ) -> intelligence_core::error::ModelResult<Option<ModelInfo>> {
        Ok(self.by_id.read().await.get(model_id).cloned())
    }
    async fn list_capabilities(
        &self,
        kind: CapabilityKind,
    ) -> intelligence_core::error::ModelResult<Vec<ModelInfo>> {
        let by_cap = self.by_capability.read().await;
        let Some(ids) = by_cap.get(&kind) else {
            return Ok(vec![]);
        };
        let by_id = self.by_id.read().await;
        let mut out = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(info) = by_id.get(id) {
                out.push(info.clone());
            }
        }
        Ok(out)
    }
    async fn refresh(&self) -> intelligence_core::error::ModelResult<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use intelligence_core::types::{CapabilityId, ModelCapability, ModelPricing, ProviderId};

    fn sample(model_id: ModelId) -> ModelInfo {
        ModelInfo {
            model_id,
            provider_id: ProviderId::new(),
            name: format!("{model_id}"),
            version: "1".into(),
            capabilities: vec![ModelCapability {
                id: CapabilityId::new(),
                name: "chat".into(),
                kind: CapabilityKind::Chat,
                input_modalities: vec!["text".into()],
                output_modalities: vec!["text".into()],
                max_input_tokens: 8192,
                max_output_tokens: 4096,
                supports_streaming: true,
                supports_tools: true,
                supports_vision: false,
                context_window: 8192,
            }],
            pricing: ModelPricing {
                currency: "USD".into(),
                input_per_million_tokens: 0.5,
                output_per_million_tokens: 1.5,
                minimum_charge: None,
                free_tier_tokens: None,
            },
            latency_p50_ms: 100,
            latency_p99_ms: 500,
            availability: AvailabilityStatus::Available,
            tags: vec!["fast".into()],
            max_batch_size: Some(32),
        }
    }

    #[tokio::test]
    async fn register_and_query() {
        let reg = DefaultModelRegistry::new();
        let id = ModelId::new();
        reg.register_model(sample(id)).await.unwrap();
        let got = reg.get(&id).await.unwrap();
        assert!(got.is_some());
        let results = reg
            .query(&ModelQuery {
                required_capabilities: vec![CapabilityKind::Chat],
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn deregister_removes() {
        let reg = DefaultModelRegistry::new();
        let id = ModelId::new();
        reg.register_model(sample(id)).await.unwrap();
        reg.deregister_model(&id).await.unwrap();
        assert!(reg.get(&id).await.unwrap().is_none());
    }
}
