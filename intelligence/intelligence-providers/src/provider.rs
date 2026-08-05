use super::error::*;
use async_trait::async_trait;
use intelligence_core::types::{
    ModelId, ModelInfo, ModelRequest, ModelResponse, ProviderId, TokenUsage,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use tokio::time::Duration;

#[derive(Debug)]
pub struct DefaultModelProvider {
    id: ProviderId,
    name: String,
    api_base: String,
    api_key_env: Option<String>,
    model: String,
    timeout_ms: u64,
    client: reqwest::Client,
}

impl DefaultModelProvider {
    pub fn new(id: ProviderId) -> Self {
        let provider = std::env::var("AI_OS_LLM_PROVIDER").unwrap_or_else(|_| "auto".into());
        let (api_base, model, name, api_key_env) = if provider == "nvapi" {
            (
                std::env::var("AI_OS_LLM_API_BASE")
                    .unwrap_or_else(|_| "https://integrate.api.nvidia.com/v1".into()),
                std::env::var("AI_OS_LLM_MODEL")
                    .unwrap_or_else(|_| "nim://meta/llama3-70b-instruct".into()),
                std::env::var("AI_OS_LLM_NAME").unwrap_or_else(|_| "nvapi".into()),
                Some("AI_OS_NVAPI_TOKEN".into()),
            )
        } else {
            (
                std::env::var("AI_OS_LLM_API_BASE")
                    .unwrap_or_else(|_| "http://localhost:11434/v1".into()),
                std::env::var("AI_OS_LLM_MODEL").unwrap_or_else(|_| "llama3.2".into()),
                std::env::var("AI_OS_LLM_NAME").unwrap_or_else(|_| "llm-provider".into()),
                Some("AI_OS_LLM_API_KEY".into()),
            )
        };
        let timeout_ms = 120_000;
        Self {
            id,
            name,
            api_base,
            api_key_env,
            model,
            timeout_ms,
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

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
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
                Ok(key) if !key.is_empty() => {
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
                _ => tracing::warn!("API key env {env} is not set or empty"),
            }
        }
        Ok(headers)
    }

    async fn post_json<T: Serialize, R: DeserializeOwned>(
        &self,
        path: &str,
        body: &T,
    ) -> ProviderResult<R> {
        let url = format!("{}{}", self.api_base.trim_end_matches('/'), path);
        tracing::debug!(url = %url, "provider request");
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

// ── OpenAI-compatible chat completion types ────────────────────

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<Message>,
    temperature: f32,
    max_tokens: u32,
    stream: bool,
}

#[derive(Serialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
    usage: Option<Usage>,
}

#[derive(Deserialize)]
struct Choice {
    message: ChoiceMessage,
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct ChoiceMessage {
    content: Option<String>,
}

#[derive(Deserialize)]
struct Usage {
    prompt_tokens: Option<u32>,
    completion_tokens: Option<u32>,
    total_tokens: Option<u32>,
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
        let input = match &request.input {
            intelligence_core::types::ModelInput::Text(t) => t.as_str(),
            _ => "",
        };

        let chat_req = ChatRequest {
            model: self.model.clone(),
            messages: vec![Message {
                role: "user".into(),
                content: input.to_string(),
            }],
            temperature: request.temperature.unwrap_or(0.1),
            max_tokens: request.max_tokens.unwrap_or(2048),
            stream: false,
        };

        let resp: ChatResponse = self.post_json("/chat/completions", &chat_req).await?;

        let content = resp
            .choices
            .into_iter()
            .next()
            .and_then(|c| c.message.content)
            .unwrap_or_default();

        let usage = resp.usage.unwrap_or(Usage {
            prompt_tokens: None,
            completion_tokens: None,
            total_tokens: None,
        });

        Ok(ModelResponse {
            request_id: request.request_id,
            model_id: ModelId::new(),
            content,
            usage: TokenUsage {
                prompt_tokens: usage.prompt_tokens.unwrap_or(0),
                completion_tokens: usage.completion_tokens.unwrap_or(0),
                total_tokens: usage.total_tokens.unwrap_or(0),
            },
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

#[cfg(test)]
mod nvapi_tests {
    use super::*;
    use intelligence_core::types::ProviderId;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_nvapi_provider_config() {
        std::env::set_var("AI_OS_LLM_PROVIDER", "nvapi");
        std::env::remove_var("AI_OS_LLM_API_BASE");
        std::env::remove_var("AI_OS_LLM_MODEL");
        std::env::remove_var("AI_OS_LLM_NAME");

        let provider = DefaultModelProvider::new(ProviderId::new());
        assert_eq!(provider.api_base, "https://api.nvidia.com/v1");
        assert_eq!(provider.model, "nim://meta/llama3-70b-instruct");
        assert_eq!(provider.name, "nvapi");
        assert_eq!(provider.api_key_env, Some("AI_OS_NVAPI_TOKEN".into()));

        std::env::remove_var("AI_OS_LLM_PROVIDER");
    }

    #[tokio::test]
    async fn test_default_provider_config() {
        std::env::remove_var("AI_OS_LLM_PROVIDER");
        std::env::remove_var("AI_OS_LLM_API_BASE");
        std::env::remove_var("AI_OS_LLM_MODEL");
        std::env::remove_var("AI_OS_LLM_NAME");

        let provider = DefaultModelProvider::new(ProviderId::new());
        assert_eq!(provider.api_base, "http://localhost:11434/v1");
        assert_eq!(provider.model, "llama3.2");
        assert_eq!(provider.name, "llm-provider");
        assert_eq!(provider.api_key_env, Some("AI_OS_LLM_API_KEY".into()));
    }
}
