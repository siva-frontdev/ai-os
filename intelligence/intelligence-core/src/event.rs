use super::types::*;
use ai_os_core::events::Event;
use serde::{Deserialize, Serialize};
use std::any::Any;

/// Event emitted when a provider is registered.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderRegistered {
    pub provider_id: ProviderId,
}

impl Event for ProviderRegistered {
    fn event_type(&self) -> &'static str {
        "intelligence.provider.registered"
    }
}

/// Event emitted when a model is registered.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRegistered {
    pub model_id: ModelId,
}

impl Event for ModelRegistered {
    fn event_type(&self) -> &'static str {
        "intelligence.model.registered"
    }
}

/// Event emitted when a request starts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestStarted {
    pub request_id: RequestId,
    pub capability: CapabilityKind,
}

impl Event for RequestStarted {
    fn event_type(&self) -> &'static str {
        "intelligence.request.started"
    }
}

/// Event emitted when a request completes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestCompleted {
    pub request_id: RequestId,
    pub model_id: ModelId,
    pub tokens_used: TokenUsage,
}

impl Event for RequestCompleted {
    fn event_type(&self) -> &'static str {
        "intelligence.request.completed"
    }
}

/// Event emitted when a request fails.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestFailed {
    pub request_id: RequestId,
    pub error: String,
}

impl Event for RequestFailed {
    fn event_type(&self) -> &'static str {
        "intelligence.request.failed"
    }
}

/// Event emitted when a fallback model is used.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FallbackUsed {
    pub request_id: RequestId,
    pub original_model: ModelId,
    pub fallback_model: ModelId,
}

impl Event for FallbackUsed {
    fn event_type(&self) -> &'static str {
        "intelligence.request.fallback_used"
    }
}

/// Event emitted when a request is served from cache.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestCached {
    pub request_id: RequestId,
}

impl Event for RequestCached {
    fn event_type(&self) -> &'static str {
        "intelligence.request.cached"
    }
}

/// Event emitted on safety violation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyViolationEvent {
    pub request_id: RequestId,
    pub rule_id: SafetyRuleId,
    pub category: SafetyCategory,
    pub detail: String,
}

impl Event for SafetyViolationEvent {
    fn event_type(&self) -> &'static str {
        "intelligence.safety.violation"
    }
}

/// Event emitted when a request/response is blocked.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyBlocked {
    pub request_id: RequestId,
    pub reason: String,
}

impl Event for SafetyBlocked {
    fn event_type(&self) -> &'static str {
        "intelligence.safety.blocked"
    }
}

/// Event emitted when cost budget warning is exceeded.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetWarning {
    pub request_id: RequestId,
    pub current_cents: f64,
    pub limit_cents: f64,
}

impl Event for BudgetWarning {
    fn event_type(&self) -> &'static str {
        "intelligence.cost.budget_warning"
    }
}

/// Event emitted when cost budget is exceeded.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetExceeded {
    pub request_id: RequestId,
    pub current_cents: f64,
    pub limit_cents: f64,
}

impl Event for BudgetExceeded {
    fn event_type(&self) -> &'static str {
        "intelligence.cost.budget_exceeded"
    }
}

/// Event emitted on cache hit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheHit {
    pub request_id: RequestId,
    pub model_id: ModelId,
}

impl Event for CacheHit {
    fn event_type(&self) -> &'static str {
        "intelligence.cache.hit"
    }
}

/// Event emitted on cache miss.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheMiss {
    pub request_id: RequestId,
    pub model_id: ModelId,
}

impl Event for CacheMiss {
    fn event_type(&self) -> &'static str {
        "intelligence.cache.miss"
    }
}

/// Event emitted on stream chunk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamChunkEvent {
    pub request_id: RequestId,
    pub chunk_index: usize,
}

impl Event for StreamChunkEvent {
    fn event_type(&self) -> &'static str {
        "intelligence.stream.chunk"
    }
}

/// Event emitted when streaming completes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamDone {
    pub request_id: RequestId,
}

impl Event for StreamDone {
    fn event_type(&self) -> &'static str {
        "intelligence.stream.done"
    }
}

/// Event emitted when coordinator is ready.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoordinatorReady;

impl Event for CoordinatorReady {
    fn event_type(&self) -> &'static str {
        "intelligence.coordinator.ready"
    }
}

/// Consumed event: core lifecycle stopping.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreLifecycleStopping;

impl Event for CoreLifecycleStopping {
    fn event_type(&self) -> &'static str {
        "core.lifecycle.stopping"
    }
}
