use super::error::*;
use async_trait::async_trait;
use intelligence_core::types::{
    ModelId, ModelInfo, ModelRequest, ModelResponse, ProviderId, RequestId, TokenUsage,
};
use std::sync::Arc;
use tokio::time::Duration;

#[derive(Debug)]
pub struct DefaultModelProvider {
    id: ProviderId,
    name: String,
    api_base: String,
    api_key_env: Option<String>,
    timeout_ms: u64,
    client: reqwest::Client,
}

impl DefaultModelProvider {
    pub fn new(id: ProviderId) -> Self {
        Self {
            id,
            name: "default-provider".into(),
            api_base: "http://localhost:8080/v1".into(),
            api_key_env: None,
            timeout_ms: 60_000,
            client: reqwest::Client::new(),
        }
    }

    pub fn with_api_base(mut self, base: impl Into<String>) -> Self {
        self.api_base = base.into();
        self
    }

    pub fn with_api_key_env(mut self, env: impl Into<String>) -> Self {
        self.api_key_env = Some(env.into());
        self
    }

    pub fn with_timeout_ms(mut self, ms: u64) -> Self {
        self.timeout_ms = ms;
        self
    }

    async fn auth_headers(&self) -> ProviderResult<reqwest::header::HeaderMap> {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::CONTENT_TYPE,
            reqwest::header::HeaderValue::from_static("application/json"),
        );
        if let Some(ref env) = self.api_key_env {
            match std::env::var(env) {
                Ok(key) => {
                    headers.insert(
                        reqwest::header::AUTHORIZATION,
                        reqwest::header::HeaderValue::from_str(&format!("Bearer {key}")).map_err(
                            |e| {
                                ProviderError::InvalidResponse(
                                    self.name.clone(),
                                    format!("invalid header: {e}"),
                                )
                            },
                        )?,
                    );
                }
                Err(_) => tracing::warn!("API key env {env} is not set"),
            }
        }
        Ok(headers)
    }

    async fn post_json<T: serde::Serialize, R: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        body: &T,
    ) -> ProviderResult<R> {
        let url = format!("{}{}", self.api_base, path);
        let headers = self.auth_headers().await?;
        let resp = self
            .client
            .post(&url)
            .headers(headers)
            .json(body)
            .timeout(Duration::from_millis(self.timeout_ms))
            .send()
            .await?;

        let status = resp.status();
        if status == 429 {
            let retry_after = resp
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(1000);
            return Err(ProviderError::RateLimited {
                provider: self.name.clone(),
                retry_after_ms: retry_after,
            });
        }
        if status.is_server_error() {
            return Err(ProviderError::ServerError {
                provider: self.name.clone(),
                status: status.as_u16(),
            });
        }
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(ProviderError::InvalidResponse(
                self.name.clone(),
                format!("HTTP {status}: {text}"),
            ));
        }
        Ok(resp.json().await?)
    }
}

#[async_trait]
impl intelligence_core::traits::ModelProvider for DefaultModelProvider {
    fn capabilities(&self) -> Vec<intelligence_core::types::ModelCapability> {
        vec![]
    }

    fn provider_id(&self) -> ProviderId {
        self.id
    }

    async fn chat(
        &self,
        request: &ModelRequest,
    ) -> intelligence_core::error::ModelResult<ModelResponse> {
        Ok(ModelResponse {
            request_id: request.request_id,
            model_id: ModelId::new(),
            content: String::new(),
            usage: TokenUsage::default(),
            finished: true,
            finish_reason: None,
        })
    }

    async fn chat_stream(
        &self,
        _request: &ModelRequest,
    ) -> intelligence_core::error::ModelResult<intelligence_core::traits::ChatStream> {
        Ok(futures::stream::empty())
    }

    async fn embed(
        &self,
        _request: &ModelRequest,
    ) -> intelligence_core::error::ModelResult<Vec<intelligence_core::types::Embedding>> {
        Ok(vec![])
    }

    async fn classify(
        &self,
        _request: &ModelRequest,
    ) -> intelligence_core::error::ModelResult<String> {
        Ok(String::new())
    }

    async fn health_check(&self) -> intelligence_core::error::ModelResult<ModelInfo> {
        Ok(ModelInfo {
            model_id: ModelId::new(),
            provider_id: self.id,
            name: self.name.clone(),
            version: "0.1.0".into(),
            capabilities: vec![],
            pricing: intelligence_core::types::ModelPricing {
                currency: "USD".into(),
                input_per_million_tokens: 0.0,
                output_per_million_tokens: 0.0,
                minimum_charge: None,
                free_tier_tokens: None,
            },
            latency_p50_ms: 200,
            latency_p99_ms: 1_000,
            availability: intelligence_core::types::AvailabilityStatus::Available,
            tags: vec![],
            max_batch_size: None,
        })
    }
}
