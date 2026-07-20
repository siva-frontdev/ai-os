use thiserror::Error;

pub type ExecutionResult<T> = Result<T, ExecutionError>;

#[derive(Debug, Error)]
pub enum ExecutionError {
    // Plan errors (1xxx)
    #[error("no suitable candidate for capability {capability}: {detail}")]
    NoSuitableCandidate { capability: String, detail: String },
    #[error("schema validation failed for {plan_id}: {reason}")]
    SchemaValidationFailed { plan_id: String, reason: String },
    #[error("sandbox resolution failed: {0}")]
    SandboxResolutionFailed(String),
    #[error("budget exceeded: {0}")]
    BudgetExceeded(String),

    // Dispatch errors (2xxx)
    #[error("dispatch queue full")]
    QueueFull,
    #[error("concurrency limit reached for backend {0}")]
    ConcurrencyLimitReached(String),
    #[error("backend unavailable: {0}")]
    BackendUnavailable(String),
    #[error("backend fallback failed: {0}")]
    BackendFallbackFailed(String),

    // Sandbox errors (3xxx)
    #[error("sandbox creation failed: {0}")]
    SandboxCreationFailed(String),
    #[error("isolation level unsupported: {0}")]
    IsolationUnsupported(String),
    #[error("sandbox profile not found: {0}")]
    ProfileNotFound(String),

    // Execution errors (4xxx)
    #[error("process spawn failed: {0}")]
    ProcessSpawnFailed(String),
    #[error("process exited with code {exit_code}")]
    InvalidExitCode { exit_code: i32 },
    #[error("process terminated by signal {signal}")]
    SignalTerminated { signal: String },
    #[error("execution panicked: {0}")]
    ExecutionPanic(String),
    #[error("WASM trap: {0}")]
    WasmTrap(String),
    #[error("container create failed: {0}")]
    ContainerCreateFailed(String),
    #[error("container start failed: {0}")]
    ContainerStartFailed(String),

    // Monitor errors (5xxx)
    #[error("timeout exceeded after {elapsed_ms}ms")]
    TimeoutExceeded { elapsed_ms: u64 },
    #[error("CPU limit exceeded: {used}ms > {max}ms")]
    CpuLimitExceeded { used: u64, max: u64 },
    #[error("memory limit exceeded: {used} > {max}")]
    MemoryLimitExceeded { used: u64, max: u64 },
    #[error("output limit exceeded: {size} > {max}")]
    OutputLimitExceeded { size: u64, max: u64 },

    // Result errors (6xxx)
    #[error("output parse failed: {0}")]
    OutputParseFailed(String),
    #[error("output too large: {size} > {max}")]
    OutputTooLarge { size: u64, max: u64 },
    #[error("artifact collection failed: {0}")]
    ArtifactCollectionFailed(String),
    #[error("enrichment failed: {0}")]
    EnrichmentFailed(String),

    // Route errors (7xxx)
    #[error("destination unavailable: {0}")]
    DestinationUnavailable(String),
    #[error("route permission denied: {0}")]
    RoutePermissionDenied(String),
    #[error("route store failed: {0}")]
    RouteStoreFailed(String),
    #[error("route pipe failed: {0}")]
    RoutePipeFailed(String),

    // Recovery errors (8xxx)
    #[error("retry budget exhausted after {attempts} attempts")]
    RetryBudgetExhausted { attempts: u32 },
    #[error("rollback step failed: {step}: {detail}")]
    RollbackStepFailed { step: String, detail: String },
    #[error("rollback retry exhausted for step {step}")]
    RollbackRetryExhausted { step: String },
    #[error("cancellation failed: {0}")]
    CancellationFailed(String),
    #[error("compensation tool failed: {0}")]
    CompensationToolFailed(String),

    // Coordinator errors (9xxx)
    #[error("pipeline channel closed: {0}")]
    ChannelClosed(String),
    #[error("stage panicked: {0}")]
    StagePanic(String),
    #[error("bypass triggered for stage {stage}: {reason}")]
    BypassTriggered { stage: String, reason: String },
    #[error("lifecycle conflict: {0}")]
    LifecycleConflict(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("configuration error: {0}")]
    ConfigurationError(String),
    #[error("permission denied: {0}")]
    PermissionDenied(String),

    // Wrapped
    #[error("core error: {0}")]
    Core(#[from] ai_os_core::CoreError),
    #[error("brain error: {0}")]
    Brain(String),
    #[error("memory error: {0}")]
    Memory(String),
    #[error("osal error: {0}")]
    Osal(String),
}

impl ExecutionError {
    pub fn brain<T: Into<String>>(msg: T) -> Self {
        Self::Brain(msg.into())
    }

    pub fn memory<T: Into<String>>(msg: T) -> Self {
        Self::Memory(msg.into())
    }

    pub fn osal<T: Into<String>>(msg: T) -> Self {
        Self::Osal(msg.into())
    }
}
