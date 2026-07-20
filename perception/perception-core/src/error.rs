use thiserror::Error;

pub type PerceptionResult<T> = Result<T, PerceptionError>;

#[derive(Debug, Error)]
pub enum PerceptionError {
    // Observer (1xxx)
    #[error("observer bind failed: {0}")]
    ObserverBindFailed(String),
    #[error("observer channel closed")]
    ObserverChannelClosed,
    #[error("observer permission denied: {0}")]
    ObserverPermissionDenied(String),
    #[error("observer read failed: {0}")]
    ObserverReadFailed(String),

    // Normalizer (2xxx)
    #[error("validation failed for {observation_id}: {reason}")]
    ValidationFailed {
        observation_id: crate::ObservationId,
        reason: String,
    },
    #[error("unsupported format: {0}")]
    UnsupportedFormat(String),
    #[error("sanitization failed: {0}")]
    SanitizationFailed(String),
    #[error("payload too big: {size} > {max}")]
    PayloadTooBig { size: u64, max: u64 },

    // Attention/Filter (3xxx)
    #[error("attention overload")]
    AttentionOverload,
    #[error("habituation state corrupt: {0}")]
    HabituationStateCorrupt(String),

    // Entity (4xxx)
    #[error("entity resolution failed: {entity_type} = {raw_value}")]
    EntityResolutionFailed {
        entity_type: String,
        raw_value: String,
    },
    #[error("entity pattern not found: {0}")]
    EntityPatternNotFound(String),
    #[error("knowledge graph query timeout")]
    KnowledgeGraphTimeout,

    // Context (5xxx)
    #[error("session resolution timeout")]
    SessionResolutionTimeout,
    #[error("session not found: {0}")]
    SessionNotFound(String),
    #[error("temporal context error: {0}")]
    TemporalContextError(String),

    // Detector (6xxx)
    #[error("state machine not found: {0:?}")]
    StateMachineNotFound(crate::ScopeId),
    #[error("state machine conflict on {scope:?}: {from} -> {to}")]
    StateMachineConflict {
        scope: crate::ScopeId,
        from: String,
        to: String,
    },
    #[error("invalid state transition on {scope:?}: {from} -> {to}")]
    InvalidStateTransition {
        scope: crate::ScopeId,
        from: String,
        to: String,
    },

    // Anomaly (7xxx)
    #[error("anomaly model not ready: {0}")]
    AnomalyModelNotReady(String),
    #[error("insufficient data for {modality:?}: need {required}, have {available}")]
    InsufficientData {
        modality: crate::Modality,
        required: usize,
        available: usize,
    },
    #[error("invalid sensitivity: {0}")]
    InvalidSensitivity(f64),

    // Fusion (8xxx)
    #[error("correlation window full")]
    CorrelationWindowFull,
    #[error("fusion conflict for {correlation_key:?}: {reason}")]
    FusionConflict {
        correlation_key: crate::CorrelationKey,
        reason: String,
    },
    #[error("correlation timeout: {0:?}")]
    CorrelationTimeout(crate::CorrelationKey),

    // Coordinator/Pipeline (9xxx)
    #[error("pipeline stage timeout: {0}")]
    PipelineStageTimeout(String),
    #[error("pipeline assembly failed: {0}")]
    PipelineAssemblyFailed(String),
    #[error("channel closed: {0}")]
    ChannelClosed(String),
    #[error("configuration error: {0}")]
    ConfigurationError(String),
    #[error("not found: {0}")]
    NotFound(String),

    // Wrapped
    #[error("core error: {0}")]
    Core(#[from] ai_os_core::CoreError),
    #[error("runtime error: {0}")]
    Runtime(String),
    #[error("memory error: {0}")]
    Memory(String),
    #[error("osal error: {0}")]
    Osal(String),
}

impl PerceptionError {
    pub fn runtime<T: Into<String>>(msg: T) -> Self {
        Self::Runtime(msg.into())
    }

    pub fn memory<T: Into<String>>(msg: T) -> Self {
        Self::Memory(msg.into())
    }

    pub fn osal<T: Into<String>>(msg: T) -> Self {
        Self::Osal(msg.into())
    }
}
