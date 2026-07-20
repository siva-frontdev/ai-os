# Execution Platform Interface — `execution` *(Phase 8)*

## Purpose

This document defines the public API surfaces of all Execution Platform crates. Each section covers one trait or major type group, its supporting types, and its complete definition. All traits are designed for `dyn` compatibility (`Send + Sync + Debug`), async operation, and loose coupling through the EventBus.

---

## Data Model

### ExecutionId — globally unique execution identifier

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ExecutionId(uuid::Uuid);
```

### ToolCapabilityId — namespaced capability identifier

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ToolCapabilityId(String);
```

### ExecutionState — finite state machine

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionState {
    Pending,
    Queued,
    Dispatching,
    Running,
    Completed,
    Failed,
    TimedOut,
    Cancelled,
    Retrying,
    RollingBack,
    Escalated,
}
```

### ExecutionBudget — resource limits

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionBudget {
    pub timeout_ms: u64,
    pub max_cpu_ms: u64,
    pub max_memory_bytes: u64,
    pub max_output_bytes: u64,
    pub max_retries: u32,
    pub deadline: Timestamp,
}
```

### ToolRequirement — abstract capability declaration from Brain

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolRequirement {
    pub capability: ToolCapabilityId,
    pub input_schema: serde_json::Value,
    pub output_expectations: Vec<OutputExpectation>,
    pub required_permissions: Vec<ExecutionPermission>,
    pub preferred_sandbox: Option<IsolationLevel>,
    pub priority: u8,
}
```

### ToolCandidate — registered, executable tool

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCandidate {
    pub tool_id: String,
    pub name: String,
    pub version: String,
    pub capability: ToolCapabilityId,
    pub binding: ToolBinding,
    pub input_schema: serde_json::Value,
    pub output_schema: Option<serde_json::Value>,
    pub side_effects: Vec<SideEffect>,
    pub sandbox_profile: SandboxProfile,
    pub metadata: HashMap<String, String>,
}
```

### ToolBinding — maps candidate to execution environment

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ToolBinding {
    Subprocess {
        binary: String,
        default_args: Vec<String>,
        env: HashMap<String, String>,
        working_dir: Option<String>,
    },
    Wasm {
        module_path: String,
        function_name: String,
        allow_net: bool,
        allow_fs: bool,
    },
    Container {
        image: String,
        command: Vec<String>,
        mounts: Vec<Mount>,
        network: NetworkConfig,
    },
}
```

### ExecutionPlan — validated, dispatch-ready plan

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPlan {
    pub execution_id: ExecutionId,
    pub tool: ToolCandidate,
    pub binding: ToolBinding,
    pub parameters: HashMap<String, serde_json::Value>,
    pub sandbox_profile: SandboxProfile,
    pub budget: ExecutionBudget,
    pub permissions: Vec<ExecutionPermission>,
    pub routing_rules: Vec<RoutingRule>,
    pub priority: u8,
}
```

### ExecutionHandle — returned on successful dispatch

```rust
#[derive(Debug)]
pub struct ExecutionHandle {
    pub execution_id: ExecutionId,
    pub plan: ExecutionPlan,
    pub state: ExecutionState,
    pub dispatched_at: Timestamp,
    pub deadline: Timestamp,
    pub cancel_tx: tokio::sync::oneshot::Sender<()>,
    pub state_rx: tokio::sync::watch::Receiver<ExecutionState>,
}
```

### ExecutionResult — produced on completion

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub execution_id: ExecutionId,
    pub state: ExecutionState,
    pub exit_code: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub artifacts: Vec<ExecutionArtifact>,
    pub parsed_output: Option<serde_json::Value>,
    pub metrics: ExecutionMetrics,
    pub error: Option<ExecutionError>,
    pub started_at: Timestamp,
    pub finished_at: Timestamp,
    pub trace_id: String,
}
```

### ExecutionMetrics — resource consumption

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ExecutionMetrics {
    pub cpu_time_ms: u64,
    pub peak_memory_bytes: u64,
    pub io_read_bytes: u64,
    pub io_write_bytes: u64,
    pub wall_clock_ms: u64,
    pub network_bytes_in: u64,
    pub network_bytes_out: u64,
}
```

### ExecutionArtifact — named byte payload

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionArtifact {
    pub name: String,
    pub content_type: String,
    pub data: Vec<u8>,
    pub size_bytes: u64,
}
```

### ExecutionPermission — scoped OS access grant

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionPermission {
    ReadFile(String),
    WriteFile(String),
    ReadDir(String),
    NetworkConnect(String),
    NetworkListen(u16),
    ProcessSpawn,
    EnvironmentRead(String),
    SystemTime,
    SystemInfo,
    DbusCall(String),
    Custom(String),
}
```

### IsolationLevel — sandbox isolation granularity

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum IsolationLevel {
    None,
    Process,
    Container,
    Wasm,
}
```

### SandboxProfile — named isolation configuration

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxProfile {
    pub name: String,
    pub isolation: IsolationLevel,
    pub filesystem: FilesystemAccess,
    pub network: NetworkAccess,
    pub allow_process_spawn: bool,
    pub capabilities: Vec<ToolCapabilityId>,
    pub seccomp_profile: Option<String>,
    pub apparmor_profile: Option<String>,
    pub read_only_rootfs: bool,
    pub tmpfs_size_bytes: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FilesystemAccess {
    None,
    ReadOnly,
    ReadWrite,
    TempOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NetworkAccess {
    None,
    OutboundOnly,
    InboundOnly,
    Full,
}
```

### RoutingRule — result delivery condition

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingRule {
    pub name: String,
    pub condition: RoutingCondition,
    pub destination: RouteDestination,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RoutingCondition {
    Always,
    OnSuccess,
    OnFailure,
    OnState(ExecutionState),
    OnExitCode(i32),
    OnOutputMatch(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RouteDestination {
    Caller,
    Memory { ttl_ms: Option<u64> },
    Brain,
    Pipe { next_tool: ToolCapabilityId },
    FileSystem { path: String },
    EventBus { event_type: String },
    Log,
    Broadcast { destinations: Vec<RouteDestination> },
}
```

### RetryPolicy — automatic retry configuration

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    pub max_retries: u32,
    pub backoff_base_ms: u64,
    pub backoff_multiplier: f64,
    pub backoff_max_ms: u64,
    pub jitter: f64,
    pub retry_on_timeout: bool,
    pub retry_on_exit_codes: Vec<i32>,
    pub max_total_timeout_ms: u64,
}
```

### RollbackPlan — compensation actions

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackPlan {
    pub execution_id: ExecutionId,
    pub compensation_steps: Vec<CompensationStep>,
    pub timeout_ms: u64,
    pub on_failure: OnRollbackFailure,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompensationStep {
    ExecuteTool { tool: ToolCapabilityId, params: HashMap<String, serde_json::Value>, description: String },
    RestoreSnapshot { snapshot_id: String, target: String },
    DeleteCreated { path: String },
    Notify { channel: String, message: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OnRollbackFailure {
    Escalate,
    IgnoreAndContinue,
    Retry(u32, u64),
}
```

---

## 1. ExecutionPlanner — plan resolution

**Crate**: `execution-planner`

**Purpose**: Accept `ToolRequirement` from the Brain, resolve it to a registered `ToolCandidate`, validate input parameters against the candidate's schema, and produce a ready-to-dispatch `ExecutionPlan`.

```rust
#[async_trait]
pub trait ExecutionPlanner: Debug + Send + Sync {
    /// Resolve a ToolRequirement into an ExecutionPlan.
    /// Returns PlanRejected error if no matching candidate or invalid input.
    async fn plan(&self, requirement: ToolRequirement) -> Result<ExecutionPlan, ExecutionError>;

    /// Validate input parameters against a candidate's schema without planning.
    async fn validate_input(&self, tool_id: &str, params: &HashMap<String, serde_json::Value>) -> Result<(), ExecutionError>;

    /// List capabilities this planner can resolve.
    fn supported_capabilities(&self) -> Vec<ToolCapabilityId>;
}
```

---

## 2. ToolRegistry — tool registration and lookup

**Crate**: `execution-registry`

**Purpose**: Thread-safe registry of all available `ToolCandidate` implementations. Supports capability-based lookup, version filtering, and runtime registration.

```rust
#[async_trait]
pub trait ToolRegistry: Debug + Send + Sync {
    /// Register a tool candidate.
    fn register(&self, candidate: ToolCandidate) -> Result<(), ExecutionError>;

    /// Unregister a tool by ID.
    fn unregister(&self, tool_id: &str) -> Result<(), ExecutionError>;

    /// Look up a tool by ID.
    fn get(&self, tool_id: &str) -> Option<ToolCandidate>;

    /// Find all tools matching a capability.
    fn find_by_capability(&self, capability: &ToolCapabilityId) -> Vec<ToolCandidate>;

    /// List all registered tool IDs.
    fn list(&self) -> Vec<String>;

    /// List all registered capabilities.
    fn capabilities(&self) -> Vec<ToolCapabilityId>;
}
```

### ToolResolver — requirement-to-candidate resolution

**Crate**: `execution-registry`

**Purpose**: Resolve a `ToolRequirement` to the best matching `ToolCandidate` by capability, version, and priority.

```rust
#[async_trait]
pub trait ToolResolver: Debug + Send + Sync {
    /// Resolve a requirement to the best matching candidate.
    async fn resolve(&self, requirement: &ToolRequirement) -> Result<ToolCandidate, ExecutionError>;

    /// Find all matching candidates for a requirement (for selection).
    async fn resolve_all(&self, requirement: &ToolRequirement) -> Vec<ToolCandidate>;
}
```

---

## 3. Dispatcher — dispatch queue and backend selection

**Crate**: `execution-dispatcher`

**Purpose**: Accept `ExecutionPlan` objects, apply concurrency limiting, select execution backend, and dispatch to the Runner. Supports priority-ordered queuing and cancellation of pending plans.

```rust
#[async_trait]
pub trait Dispatcher: Debug + Send + Sync {
    /// Submit an execution plan for dispatch.
    /// Returns a handle for status tracking and cancellation.
    async fn submit(&self, plan: ExecutionPlan) -> Result<ExecutionHandle, ExecutionError>;

    /// Cancel a pending or running execution.
    async fn cancel(&self, execution_id: ExecutionId) -> Result<(), ExecutionError>;

    /// Get the current status of an execution.
    async fn status(&self, execution_id: ExecutionId) -> Result<Option<ExecutionState>, ExecutionError>;

    /// Get queue depth and concurrency stats.
    fn stats(&self) -> DispatchStats;

    /// Get the current dispatch queue contents.
    fn queue(&self) -> Vec<PendingExecution>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DispatchStats {
    pub active_executions: usize,
    pub queued_executions: usize,
    pub max_concurrency: usize,
    pub total_dispatched: u64,
    pub total_rejected: u64,
    pub by_backend: HashMap<String, usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingExecution {
    pub execution_id: ExecutionId,
    pub tool_id: String,
    pub priority: u8,
    pub submitted_at: Timestamp,
}
```

---

## 4. Runner — execution backends

**Crate**: `execution-runner`

**Purpose**: Execute a `ToolCandidate` binding in its configured environment (subprocess, WASM, container). Manages process lifecycle, I/O streams, and resource collection.

```rust
#[async_trait]
pub trait Runner: Debug + Send + Sync {
    /// Start executing a plan. Returns a handle for lifecycle management.
    async fn start(&self, plan: &ExecutionPlan) -> Result<RunnerHandle, ExecutionError>;

    /// Cancel a running execution.
    async fn cancel(&self, handle: &RunnerHandle) -> Result<(), ExecutionError>;

    /// Check if the backend is healthy and available.
    async fn health(&self) -> Result<(), ExecutionError>;

    /// Get backend configuration summary.
    fn config(&self) -> RunnerConfig;
}

#[derive(Debug)]
pub struct RunnerHandle {
    pub execution_id: ExecutionId,
    pub backend: ExecutionBackend,
    pub pid: Option<u32>,
    pub started_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionBackend {
    Subprocess,
    Wasm,
    Container,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerConfig {
    pub backend: ExecutionBackend,
    pub max_concurrent: usize,
    pub default_timeout_ms: u64,
}
```

---

## 5. SandboxEnforcer — sandbox policy resolution

**Crate**: `execution-sandbox`

**Purpose**: Given a `ToolCandidate` and execution context, resolve the required `SandboxProfile`. Validate that the requested isolation level is achievable.

```rust
#[async_trait]
pub trait SandboxEnforcer: Debug + Send + Sync {
    /// Resolve the sandbox profile for a given tool and context.
    async fn resolve_profile(&self, candidate: &ToolCandidate, context: &ExecutionContext) -> Result<SandboxProfile, ExecutionError>;

    /// Register a sandbox profile.
    fn register_profile(&self, profile: SandboxProfile) -> Result<(), ExecutionError>;

    /// Check if an isolation level is supported.
    fn supports_isolation(&self, level: IsolationLevel) -> bool;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionContext {
    pub session_id: String,
    pub user_id: String,
    pub tool_capability: ToolCapabilityId,
    pub permissions: Vec<ExecutionPermission>,
    pub sandbox_profile: Option<SandboxProfile>,
}
```

---

## 6. ExecutionMonitor — runtime monitoring

**Crate**: `execution-monitor`

**Purpose**: Tracks running executions, enforces timeouts and resource limits, provides health probes.

```rust
#[async_trait]
pub trait ExecutionMonitor: Debug + Send + Sync {
    /// Register an execution for monitoring.
    async fn watch(&self, handle: &ExecutionHandle) -> Result<(), ExecutionError>;

    /// Update resource usage for an execution.
    async fn update_resources(&self, execution_id: ExecutionId, resources: ExecutionMetrics) -> Result<(), ExecutionError>;

    /// Cancel an execution (triggered by timeout or resource violation).
    async fn cancel(&self, execution_id: ExecutionId, reason: &str) -> Result<(), ExecutionError>;

    /// Get current resource usage for an execution.
    async fn resource_usage(&self, execution_id: ExecutionId) -> Result<Option<ExecutionMetrics>, ExecutionError>;

    /// Unregister a completed execution from monitoring.
    async fn unwatch(&self, execution_id: ExecutionId) -> Result<(), ExecutionError>;

    /// Health check for the monitoring subsystem.
    async fn health(&self) -> Result<(), ExecutionError>;
}
```

---

## 7. ResultCollector — output collection and parsing

**Crate**: `execution-results`

**Purpose**: Collect stdout/stderr from completed executions, parse structured output, attach artifacts, produce `ExecutionResult`.

```rust
#[async_trait]
pub trait ResultCollector: Debug + Send + Sync {
    /// Collect output and produce an ExecutionResult.
    async fn collect(&self, execution_id: ExecutionId, stdout: Vec<u8>, stderr: Vec<u8>, artifacts: Vec<ExecutionArtifact>) -> Result<ExecutionResult, ExecutionError>;

    /// Parse stdout bytes into structured data.
    async fn parse_output(&self, data: &[u8], format: &str) -> Result<Option<serde_json::Value>, ExecutionError>;

    /// Register an output parser for a content type.
    fn register_parser(&self, content_type: &str, parser: Box<dyn OutputParser>);
}

#[async_trait]
pub trait OutputParser: Debug + Send + Sync {
    fn content_type(&self) -> &str;
    async fn parse(&self, data: &[u8]) -> Result<serde_json::Value, ExecutionError>;
}
```

### OutputRouter — result distribution

**Crate**: `execution-results`

**Purpose**: Route execution results to configured destinations based on routing rules.

```rust
#[async_trait]
pub trait OutputRouter: Debug + Send + Sync {
    /// Register a routing rule.
    fn register_rule(&self, rule: RoutingRule) -> Result<(), ExecutionError>;

    /// Route an execution result to matching destinations.
    async fn route(&self, result: &ExecutionResult) -> Result<Vec<RouteResult>, ExecutionError>;

    /// List all registered routing rules.
    fn list_rules(&self) -> Vec<RoutingRule>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteResult {
    pub rule: String,
    pub destination: RouteDestination,
    pub success: bool,
    pub error: Option<String>,
}
```

---

## 8. RecoveryManager — retry and rollback

**Crate**: `execution-recovery`

**Purpose**: Manage automatic retry with backoff and rollback with compensation actions.

```rust
#[async_trait]
pub trait RecoveryManager: Debug + Send + Sync {
    /// Register an execution for recovery (with retry policy and optional rollback plan).
    async fn register(&self, execution_id: ExecutionId, retry_policy: RetryPolicy, rollback_plan: Option<RollbackPlan>) -> Result<(), ExecutionError>;

    /// Notify recovery manager of a completed/failed execution.
    async fn notify(&self, result: &ExecutionResult) -> Result<RecoveryDecision, ExecutionError>;

    /// Execute the rollback plan for an execution.
    async fn rollback(&self, execution_id: ExecutionId) -> Result<(), ExecutionError>;

    /// Get current recovery state.
    async fn recovery_state(&self, execution_id: ExecutionId) -> Result<Option<RecoveryState>, ExecutionError>;

    /// List all active recovery states.
    fn active_recoveries(&self) -> Vec<ExecutionId>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecoveryDecision {
    None,
    Retry { attempt: u32, backoff_ms: u64 },
    Rollback,
    Escalate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecoveryState {
    Idle,
    Retrying(u32),
    RollingBack,
    Escalated,
    Resolved,
}
```

---

## 9. PipelineManager — stage registration and wiring

**Crate**: `execution-coordinator`

**Purpose**: Manages the internal pipeline topology: creates bounded channels between stages, registers stage handles, monitors channel fill levels, and handles bypass mode transitions.

```rust
#[async_trait]
pub trait PipelineManager: Debug + Send + Sync {
    /// Register a pipeline stage with channel config.
    fn register_stage(&self, name: &str, config: StageChannelConfig) -> Result<(), ExecutionError>;

    /// Connect two stages with a bounded mpsc channel.
    fn connect(&self, from: &str, to: &str, capacity: usize) -> Result<(), ExecutionError>;

    /// Get channel fill levels.
    fn channel_fill_levels(&self) -> HashMap<String, (usize, usize)>;

    /// Place a stage into bypass mode.
    fn set_bypass(&self, stage: &str, bypass: bool) -> Result<(), ExecutionError>;

    /// Remove a stage from the pipeline.
    fn remove_stage(&self, name: &str) -> Result<(), ExecutionError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageChannelConfig {
    pub name: String,
    pub input_capacity: usize,
    pub output_capacity: usize,
    pub timeout_ms: u64,
}
```

---

## 10. ExecutionCoordinator — top-level orchestration

**Crate**: `execution-coordinator`

**Purpose**: Assemble the pipeline, manage lifecycle, monitor health, handle reconfiguration, and bridge to the Brain Platform. Implements Core's `Service` trait.

```rust
#[async_trait]
pub trait ExecutionCoordinator: Debug + Send + Sync {
    /// Submit a ToolRequirement from the Brain for execution.
    async fn submit(&self, requirement: ToolRequirement) -> Result<ExecutionHandle, ExecutionError>;

    /// Cancel an execution by ID.
    async fn cancel(&self, execution_id: ExecutionId) -> Result<(), ExecutionError>;

    /// Get execution status.
    async fn status(&self, execution_id: ExecutionId) -> Result<Option<ExecutionResult>, ExecutionError>;

    /// Start the execution pipeline.
    async fn start(&self) -> Result<(), ExecutionError>;

    /// Stop the execution pipeline gracefully.
    async fn stop(&self) -> Result<(), ExecutionError>;

    /// Reload configuration at runtime.
    async fn reload_config(&self, config: CoordinatorConfig) -> Result<(), ExecutionError>;

    /// Get pipeline status.
    async fn pipeline_status(&self) -> PipelineStatus;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineStatus {
    pub running: bool,
    pub stages: Vec<StageStatus>,
    pub total_executions: u64,
    pub active_executions: u64,
    pub queued_executions: u64,
    pub uptime_seconds: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageStatus {
    pub name: String,
    pub active: bool,
    pub bypass: bool,
    pub plans_in: u64,
    pub plans_out: u64,
    pub errors: u64,
    pub p99_latency_us: f64,
}
```

---

## 11. ExecutionHistory — immutable record for Memory

**Crate**: `execution-core`

**Purpose**: Immutable record of a completed execution for storage in the Memory Platform. Produced by the ResultCollector and consumed by the Memory Platform.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionHistory {
    pub execution_id: ExecutionId,
    pub tool_id: String,
    pub capability: ToolCapabilityId,
    pub state: ExecutionState,
    pub exit_code: Option<i32>,
    pub input_params: HashMap<String, serde_json::Value>,
    pub output_summary: OutputSummary,
    pub artifacts_summary: Vec<ArtifactSummary>,
    pub metrics: ExecutionMetrics,
    pub error: Option<String>,
    pub retry_attempts: u32,
    pub started_at: Timestamp,
    pub finished_at: Timestamp,
    pub trace_id: String,
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputSummary {
    pub stdout_size_bytes: u64,
    pub stderr_size_bytes: u64,
    pub parsed_output_type: Option<String>,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactSummary {
    pub name: String,
    pub content_type: String,
    pub size_bytes: u64,
}
```

---

## 12. ExecutionSession — scoped execution context

**Crate**: `execution-coordinator`

**Purpose**: Scoped execution context that binds a Brain request to a pipeline flow. Created by the Coordinator for each incoming `ToolRequirement`.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionSession {
    pub session_id: String,
    pub execution_id: ExecutionId,
    pub brain_request_id: Option<String>,
    pub runtime_task_id: Option<String>,
    pub trace_id: String,
    pub budget: ExecutionBudget,
    pub permissions: Vec<ExecutionPermission>,
    pub started_at: Timestamp,
    pub state: ExecutionState,
}
```

---

## Error Model

```rust
#[derive(Debug, thiserror::Error)]
pub enum ExecutionError {
    // Planner (1xxx)
    #[error("no suitable candidate for capability {0:?}")]
    NoSuitableCandidate(ToolCapabilityId),
    #[error("invalid input for {tool}: {reason}")]
    InvalidInput { tool: String, reason: String },
    #[error("schema validation failed for {tool}: {errors:?}")]
    SchemaValidationFailed { tool: String, errors: Vec<String> },

    // Dispatcher (2xxx)
    #[error("max concurrency reached")]
    MaxConcurrencyReached,
    #[error("dispatch queue full (priority {priority}, queue {queue_size})")]
    QueueFull { priority: u8, queue_size: usize },
    #[error("backend selection failed: {0}")]
    BackendSelectionFailed(String),
    #[error("dispatch rejected: {0}")]
    DispatchRejected(String),

    // Runner (3xxx)
    #[error("subprocess spawn failed: {0}")]
    SubprocessSpawnFailed(String),
    #[error("subprocess kill failed: {0}")]
    SubprocessKillFailed(String),
    #[error("WASM compilation failed: {0}")]
    WasmCompilationFailed(String),
    #[error("WASM execution failed: {0}")]
    WasmExecutionFailed(String),
    #[error("container pull failed: {0}")]
    ContainerPullFailed(String),
    #[error("container start failed: {0}")]
    ContainerStartFailed(String),
    #[error("container exec failed: {0}")]
    ContainerExecFailed(String),
    #[error("backend unavailable: {0}")]
    BackendUnavailable(String),

    // Sandbox (4xxx)
    #[error("sandbox creation failed: {0}")]
    SandboxCreationFailed(String),
    #[error("sandbox violation ({policy}): {detail}")]
    SandboxViolation { policy: String, detail: String },
    #[error("isolation level not supported: {0:?}")]
    IsolationLevelNotSupported(IsolationLevel),

    // Monitor (5xxx)
    #[error("timeout exceeded for {execution_id:?} (limit {timeout_ms}ms)")
    TimeoutExceeded { execution_id: ExecutionId, timeout_ms: u64 },
    #[error("memory limit exceeded: {peak_bytes} > {limit_bytes}")]
    MemoryLimitExceeded { peak_bytes: u64, limit_bytes: u64 },
    #[error("output limit exceeded: {output_bytes} > {limit_bytes}")]
    OutputLimitExceeded { output_bytes: u64, limit_bytes: u64 },
    #[error("CPU limit exceeded: {cpu_ms} > {limit_ms}")]
    CpuLimitExceeded { cpu_ms: u64, limit_ms: u64 },

    // Results (6xxx)
    #[error("output parse failed ({format}): {error}")]
    OutputParseFailed { format: String, error: String },
    #[error("output truncated: {captured_bytes} of {limit_bytes} bytes")]
    OutputTruncated { captured_bytes: u64, limit_bytes: u64 },
    #[error("artifact store failed: {0}")]
    ArtifactStoreFailed(String),
    #[error("route failed to {destination}: {error}")]
    RouteFailed { destination: String, error: String },

    // Recovery (7xxx)
    #[error("retry exhausted for {execution_id:?} after {attempts} attempts")]
    RetryExhausted { execution_id: ExecutionId, attempts: u32 },
    #[error("rollback failed for {execution_id:?}: {reason}")]
    RollbackFailed { execution_id: ExecutionId, reason: String },
    #[error("rollback plan not found: {0:?}")]
    RollbackPlanNotFound(ExecutionId),
    #[error("recovery in progress: {0:?}")]
    RecoveryInProgress(ExecutionId),

    // Coordinator (8xxx)
    #[error("pipeline assembly failed: {0}")]
    PipelineAssemblyFailed(String),
    #[error("pipeline stage timeout: {0}")]
    StageTimeout(String),
    #[error("channel closed: {0}")]
    ChannelClosed(String),
    #[error("configuration error: {0}")]
    ConfigurationError(String),

    // Wrapped
    #[error("core error: {0}")]
    Core(#[from] core::CoreError),
    #[error("runtime error: {0}")]
    Runtime(#[from] runtime::RuntimeError),
    #[error("memory error: {0}")]
    Memory(#[from] memory_core::MemoryError),
    #[error("osal error: {0}")]
    Osal(#[from] osal_core::OsalError),
    #[error("perception error: {0}")]
    Perception(#[from] perception_core::PerceptionError),
}
```

---

## Events

### Published Events

| Event | Type String | Payload | Stage |
|---|---|---|---|
| `ExecutionPlanCreated` | `execution.plan.created` | `{ execution_id, capability, tool_id }` | Planner |
| `ExecutionPlanRejected` | `execution.plan.rejected` | `{ capability, reason }` | Planner |
| `ExecutionDispatched` | `execution.dispatched` | `{ execution_id, backend }` | Dispatcher |
| `ExecutionQueued` | `execution.queued` | `{ execution_id, priority, queue_depth }` | Dispatcher |
| `ExecutionStarted` | `execution.started` | `{ execution_id, backend, pid }` | Runner |
| `ExecutionCompleted` | `execution.completed` | `{ execution_id, exit_code, metrics }` | Runner |
| `ExecutionFailed` | `execution.failed` | `{ execution_id, error, stage }` | Runner/Monitor |
| `ExecutionTimedOut` | `execution.timed_out` | `{ execution_id, timeout_ms, elapsed_ms }` | Monitor |
| `ExecutionCancelled` | `execution.cancelled` | `{ execution_id, reason }` | Monitor |
| `ExecutionRetrying` | `execution.retrying` | `{ execution_id, attempt, backoff_ms }` | Recovery |
| `ExecutionRollingBack` | `execution.rolling_back` | `{ execution_id, step }` | Recovery |
| `ExecutionRolledBack` | `execution.rolled_back` | `{ execution_id }` | Recovery |
| `ExecutionEscalated` | `execution.escalated` | `{ execution_id, reason }` | Recovery |
| `ExecutionResultRouted` | `execution.result.routed` | `{ execution_id, destinations }` | Results |
| `ExecutionArtifactStored` | `execution.artifact.stored` | `{ execution_id, name, size }` | Results |
| `ResourceLimitExceeded` | `execution.resource.exceeded` | `{ execution_id, limit_type, actual, max }` | Monitor |
| `BackpressureWarning` | `execution.backpressure` | `{ queue_depth, capacity }` | Dispatcher |
| `PipelineStageFailed` | `execution.pipeline.stage_failed` | `{ stage, error, restarts }` | Coordinator |
| `CoordinatorStarted` | `execution.coordinator.started` | `{ config_summary }` | Coordinator |
| `CoordinatorStopped` | `execution.coordinator.stopped` | `{ uptime_seconds, total_executions }` | Coordinator |

### Consumed Events

| Source | Event | Consumer | Purpose |
|---|---|---|---|
| Brain | `brain.decision.made` | Coordinator | Submit ToolRequirement |
| Brain | `brain.plan.step_ready` | Coordinator | Submit plan step |
| Runtime | `runtime.task_cancelled` | Monitor | Cancel associated execution |
| Runtime | `runtime.session_destroyed` | Monitor | Cancel session-scoped executions |
| Core | `core.config_changed` | Coordinator | Reload configuration |
| Core | `core.service_stopped` | Coordinator | Graceful shutdown |
| Perception | `perception.observation.ready` | Results (future) | Enrich results |

---

## Dependencies

| Crate | Depends On |
|---|---|
| `execution-core` | `ai-os-core`, `memory-core`, `serde`, `uuid`, `thiserror`, `tokio` |
| `execution-registry` | `execution-core` |
| `execution-planner` | `execution-core`, `execution-registry` |
| `execution-sandbox` | `execution-core` |
| `execution-runner` | `execution-core`, `execution-sandbox`, `osal-core`, `osal-process`, `osal-filesystem`, `osal-network` |
| `execution-dispatcher` | `execution-core`, `execution-registry`, `execution-runner`, `ai-os-runtime`, `tokio` |
| `execution-monitor` | `execution-core`, `execution-runner`, `ai-os-runtime`, `tokio` |
| `execution-results` | `execution-core`, `execution-runner`, `memory-core`, `serde_json` |
| `execution-recovery` | `execution-core`, `execution-monitor`, `execution-results` |
| `execution-coordinator` | All execution crates, `ai-os-runtime`, `memory-core`, `perception-core`, `brain-core`, `tokio` |

---

## Performance Targets

| Operation | Target Latency (p50/p99) | Sustained Throughput |
|---|---|---|
| Plan resolution (cached) | 2 µs / 10 µs | 200K/s |
| Dispatch (no queue) | 5 µs / 20 µs | 100K/s |
| Subprocess spawn (empty) | 200 µs / 1 ms | 5K/s |
| WASM instantiate + run | 100 µs / 500 µs | 10K/s |
| Output collection (1 KB) | 5 µs / 20 µs | 100K/s |
| Output parsing (JSON, 1 KB) | 10 µs / 50 µs | 50K/s |
| Result routing (1 dest) | 5 µs / 20 µs | 100K/s |
| Retry evaluation | 1 µs / 5 µs | 500K/s |
| End-to-end (subprocess) | 500 µs / 5 ms | 2K/s |
| End-to-end (WASM) | 300 µs / 3 ms | 3K/s |
| End-to-end (container, warm) | 1 s / 5 s | 50/s |

---

## References

- [Execution Platform Architecture](../architecture/execution.md)
- [Execution Platform Configuration](../configuration/execution.md)
- [Execution Pipeline Design](../execution-pipeline.md)
- [RFC-0005: Execution Platform](../rfc/RFC-0005-execution-platform.md)
