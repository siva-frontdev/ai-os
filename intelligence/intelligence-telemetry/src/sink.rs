use async_trait::async_trait;
use intelligence_core::error::ModelResult;
use intelligence_core::traits::TelemetrySink;
use intelligence_core::types::{MetricsSnapshot, RequestId, SafetyEvent, TelemetryRecord};
use std::collections::HashMap;
use tokio::sync::RwLock;

#[derive(Debug, Default)]
pub struct DefaultTelemetrySink {
    requests: RwLock<HashMap<String, TelemetryRecord>>,
    errors: RwLock<HashMap<String, String>>,
}

impl DefaultTelemetrySink {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl TelemetrySink for DefaultTelemetrySink {
    async fn record_request(&self, record: TelemetryRecord) {
        self.requests
            .write()
            .await
            .insert(record.request_id.to_string(), record);
    }
    async fn record_safety_event(&self, event: SafetyEvent, request_id: &RequestId) {
        let key = format!("{}:{}", request_id, event.rule_id);
        self.errors.write().await.insert(key, event.detail);
    }
    async fn record_error(
        &self,
        error: &intelligence_core::error::ModelError,
        request_id: &RequestId,
    ) {
        self.errors
            .write()
            .await
            .insert(request_id.to_string(), error.to_string());
    }
    async fn export_metrics(&self) -> ModelResult<MetricsSnapshot> {
        Ok(MetricsSnapshot {
            total_requests: 0,
            total_latency_ms_sum: 0,
            total_tokens_in: 0,
            total_tokens_out: 0,
            total_cost_cents: 0.0,
            cache_hit_rate: 0.0,
            safety_event_count: 0,
            error_rate: 0.0,
            per_provider: HashMap::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn smoke() {
        let _r = DefaultTelemetrySink::new();
    }
}
