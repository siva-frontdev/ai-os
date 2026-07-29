use memory_core::Timestamp;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

use tokio::sync::{oneshot, watch};

// ── ExecutionId ────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ExecutionId(uuid::Uuid);

impl ExecutionId {
    pub fn new() -> Self {
        Self(uuid::Uuid::now_v7())
    }

    pub const fn from_uuid(uuid: uuid::Uuid) -> Self {
        Self(uuid)
    }

    pub const fn as_uuid(&self) -> &uuid::Uuid {
        &self.0
    }

    pub fn into_uuid(self) -> uuid::Uuid {
        self.0
    }
}

impl Default for ExecutionId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ExecutionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for ExecutionId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        uuid::Uuid::from_str(s).map(Self)
    }
}

// ── ExecutionState (14 states) ─────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ExecutionState {
    Planned,
    Queued,
    Dispatching,
    Running,
    Collecting,
    Routing,
    Completed,
    Failed,
    TimedOut,
    Cancelled,
    Retrying,
    RollingBack,
    RolledBack,
    Escalated,
}

impl ExecutionState {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Completed
                | Self::Failed
                | Self::TimedOut
                | Self::Cancelled
                | Self::RolledBack
                | Self::Escalated
        )
    }

    pub fn is_active(&self) -> bool {
        matches!(
            self,
            Self::Planned
                | Self::Queued
                | Self::Dispatching
                | Self::Running
                | Self::Collecting
                | Self::Routing
                | Self::Retrying
                | Self::RollingBack
        )
    }

    pub fn can_cancel(&self) -> bool {
        matches!(
            self,
            Self::Planned | Self::Queued | Self::Dispatching | Self::Running | Self::Collecting
        )
    }
}

impl fmt::Display for ExecutionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Planned => write!(f, "Planned"),
            Self::Queued => write!(f, "Queued"),
            Self::Dispatching => write!(f, "Dispatching"),
            Self::Running => write!(f, "Running"),
            Self::Collecting => write!(f, "Collecting"),
            Self::Routing => write!(f, "Routing"),
            Self::Completed => write!(f, "Completed"),
            Self::Failed => write!(f, "Failed"),
            Self::TimedOut => write!(f, "TimedOut"),
            Self::Cancelled => write!(f, "Cancelled"),
            Self::Retrying => write!(f, "Retrying"),
            Self::RollingBack => write!(f, "RollingBack"),
            Self::RolledBack => write!(f, "RolledBack"),
            Self::Escalated => write!(f, "Escalated"),
        }
    }
}

// ── ExecutionPriority ──────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ExecutionPriority(u8);

impl ExecutionPriority {
    pub const CRITICAL: Self = Self(0);
    pub const HIGH: Self = Self(25);
    pub const NORMAL: Self = Self(50);
    pub const LOW: Self = Self(75);
    pub const BACKGROUND: Self = Self(100);

    pub fn new(value: u8) -> Self {
        Self(value.min(100))
    }

    pub fn value(&self) -> u8 {
        self.0
    }
}

impl Default for ExecutionPriority {
    fn default() -> Self {
        Self::NORMAL
    }
}

// ── ToolBinding ────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ToolBinding {
    Subprocess {
        binary: String,
        args: Vec<String>,
        env: HashMap<String, String>,
        working_dir: Option<String>,
        allowed_paths: Vec<String>,
        denied_binaries: Vec<String>,
    },
    Wasm {
        module_path: String,
        function_name: String,
        input_format: WasmInputFormat,
        fuel_limit: Option<u64>,
        precompile: bool,
    },
    Container {
        image: String,
        command: Vec<String>,
        mounts: Vec<ContainerMount>,
        network: ContainerNetwork,
        pull_policy: PullPolicy,
        resource_limits: ContainerResources,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WasmInputFormat {
    Json,
    FlatBuffers,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContainerMount {
    pub source: String,
    pub target: String,
    pub read_only: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ContainerNetwork {
    None,
    Host,
    Bridge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PullPolicy {
    Always,
    IfNotPresent,
    Never,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContainerResources {
    pub cpu_shares: Option<u64>,
    pub memory_limit_bytes: Option<u64>,
    pub pids_limit: Option<i64>,
}

impl Default for ContainerResources {
    fn default() -> Self {
        Self {
            cpu_shares: None,
            memory_limit_bytes: None,
            pids_limit: None,
        }
    }
}

// ── SandboxProfile ─────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SandboxProfile {
    pub name: String,
    pub isolation: IsolationLevel,
    pub allowed_paths: Vec<String>,
    pub denied_paths: Vec<String>,
    pub allowed_syscalls: Vec<String>,
    pub allowed_capabilities: Vec<String>,
    pub env_whitelist: Vec<String>,
    pub env_blacklist: Vec<String>,
    pub network_access: bool,
    pub max_files: Option<u64>,
    pub max_processes: Option<u64>,
}

impl SandboxProfile {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            isolation: IsolationLevel::Process,
            allowed_paths: Vec::new(),
            denied_paths: Vec::new(),
            allowed_syscalls: Vec::new(),
            allowed_capabilities: Vec::new(),
            env_whitelist: Vec::new(),
            env_blacklist: Vec::new(),
            network_access: false,
            max_files: None,
            max_processes: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum IsolationLevel {
    None,
    Process,
    Wasm,
    Container,
}

// ── ExecutionBudget ────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionBudget {
    pub timeout_ms: u64,
    pub max_cpu_ms: u64,
    pub max_memory_bytes: u64,
    pub max_output_bytes: u64,
    pub deadline: Option<Timestamp>,
}

impl ExecutionBudget {
    pub fn new(timeout_ms: u64) -> Self {
        Self {
            timeout_ms,
            max_cpu_ms: timeout_ms,
            max_memory_bytes: 512 * 1024 * 1024,
            max_output_bytes: 10 * 1024 * 1024,
            deadline: None,
        }
    }
}

impl Default for ExecutionBudget {
    fn default() -> Self {
        Self::new(30_000)
    }
}

// ── ExecutionPermissions ───────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionPermissions {
    pub allowed_backends: Vec<String>,
    pub max_priority: ExecutionPriority,
    pub allow_network: bool,
    pub allow_filesystem_write: bool,
    pub allow_privileged: bool,
    pub required_approval: bool,
}

impl Default for ExecutionPermissions {
    fn default() -> Self {
        Self {
            allowed_backends: vec!["subprocess".into(), "wasm".into()],
            max_priority: ExecutionPriority::NORMAL,
            allow_network: false,
            allow_filesystem_write: false,
            allow_privileged: false,
            required_approval: false,
        }
    }
}

// ── ExecutionSession ───────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionSession {
    pub session_id: String,
    pub user_id: String,
    pub roles: Vec<String>,
    pub permissions: ExecutionPermissions,
}

// ── ExecutionContext ───────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionContext {
    pub session: ExecutionSession,
    pub trace_id: String,
    pub span_id: String,
    pub originating_goal: Option<String>,
    pub originating_plan: Option<String>,
}

// ── ExecutionRequest ───────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionRequest {
    pub requirement_id: String,
    pub capability_id: String,
    pub inputs: HashMap<String, String>,
    pub context: ExecutionContext,
    pub budget: ExecutionBudget,
    pub priority: ExecutionPriority,
    pub retry_policy: RetryPolicy,
}

// ── RetryPolicy ────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RetryPolicy {
    pub max_retries: u32,
    pub base_delay_ms: u64,
    pub multiplier: f64,
    pub max_delay_ms: u64,
}

impl RetryPolicy {
    pub fn none() -> Self {
        Self {
            max_retries: 0,
            base_delay_ms: 0,
            multiplier: 1.0,
            max_delay_ms: 0,
        }
    }

    pub fn compute_backoff(&self, attempt: u32) -> u64 {
        if attempt == 0 {
            return 0;
        }
        let delay = (self.base_delay_ms as f64 * self.multiplier.powi(attempt as i32 - 1)) as u64;
        delay.min(self.max_delay_ms)
    }
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay_ms: 1000,
            multiplier: 2.0,
            max_delay_ms: 30_000,
        }
    }
}

// ── ExecutionPlan ──────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionPlan {
    pub id: ExecutionId,
    pub request: ExecutionRequest,
    pub binding: ToolBinding,
    pub sandbox_profile: SandboxProfile,
    pub budget: ExecutionBudget,
    pub priority: ExecutionPriority,
    pub permissions: ExecutionPermissions,
    pub routing_rules: Vec<RoutingRule>,
    pub rollback_plan: Option<RollbackPlan>,
    pub state: ExecutionState,
    pub created_at: Timestamp,
}

// ── RoutingRule ────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoutingRule {
    pub condition: String,
    pub destinations: Vec<RouteDestination>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RouteDestination {
    Caller,
    Memory,
    Brain,
    Pipe(String),
    FileSystem(String),
    EventBus(String),
    Log,
    Broadcast(Vec<RouteDestination>),
}

// ── RollbackPlan ───────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RollbackPlan {
    pub steps: Vec<CompensationStep>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompensationStep {
    pub step_type: CompensationStepType,
    pub description: String,
    pub timeout_ms: u64,
    pub on_failure: OnRollbackFailure,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CompensationStepType {
    ExecuteTool {
        capability: String,
        inputs: HashMap<String, String>,
    },
    RestoreSnapshot {
        snapshot_id: String,
    },
    DeleteCreated {
        path: String,
    },
    Notify {
        channel: String,
        message: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OnRollbackFailure {
    Escalate,
    IgnoreAndContinue,
    Retry(u32, u64),
}

// ── ExecutionHandle ────────────────────────────────────────

pub struct ExecutionHandle {
    pub id: ExecutionId,
    pub state_rx: watch::Receiver<ExecutionState>,
    pub cancel_tx: oneshot::Sender<CancelReason>,
    pub result_rx: oneshot::Receiver<ExecutionResult>,
}

impl fmt::Debug for ExecutionHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExecutionHandle")
            .field("id", &self.id)
            .field("state", &self.state_rx.borrow().clone())
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CancelReason {
    UserRequested,
    Timeout,
    ResourceLimitExceeded,
    SessionClosed,
    Shutdown,
    DependencyFailed,
    Other(String),
}

// ── ExecutionResult ────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub execution_id: ExecutionId,
    pub state: ExecutionState,
    pub exit_code: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub parsed_output: Option<serde_json::Value>,
    pub artifacts: Vec<ExecutionArtifact>,
    pub metrics: ExecutionMetrics,
    pub error: Option<String>,
    pub routing_results: Vec<RouteResult>,
    pub started_at: Timestamp,
    pub completed_at: Timestamp,
}

// ── ExecutionArtifact ──────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionArtifact {
    pub name: String,
    pub mime_type: String,
    pub size_bytes: u64,
    pub data: Vec<u8>,
}

// ── ExecutionMetrics ───────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ExecutionMetrics {
    pub wall_clock_ms: u64,
    pub cpu_ms: u64,
    pub peak_memory_bytes: u64,
    pub io_read_bytes: u64,
    pub io_write_bytes: u64,
}

impl Default for ExecutionMetrics {
    fn default() -> Self {
        Self {
            wall_clock_ms: 0,
            cpu_ms: 0,
            peak_memory_bytes: 0,
            io_read_bytes: 0,
            io_write_bytes: 0,
        }
    }
}

// ── ExecutionHistory (persistent record) ───────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionHistory {
    pub execution_id: ExecutionId,
    pub plan_snapshot: ExecutionPlan,
    pub result: Option<ExecutionResult>,
    pub state_transitions: Vec<StateTransition>,
    pub events: Vec<ExecutionEventRecord>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StateTransition {
    pub from: ExecutionState,
    pub to: ExecutionState,
    pub timestamp: Timestamp,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionEventRecord {
    pub event_type: String,
    pub payload_json: String,
    pub timestamp: Timestamp,
}

// ── RouteResult ────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RouteResult {
    pub destination: String,
    pub success: bool,
    pub error: Option<String>,
}

// ── ExecutionCapability ────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionCapability {
    pub id: String,
    pub name: String,
    pub description: String,
    pub input_schema: HashMap<String, String>,
    pub output_schema: HashMap<String, String>,
    pub required_permissions: Vec<String>,
}

// ── CapabilityOrigin ──────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CapabilityOrigin {
    Native,
    Learned,
    DiscoveredLocal,
    DiscoveredExternal,
    Generated,
    Deprecated,
    Disabled,
    Experimental,
}

impl CapabilityOrigin {
    pub fn trust_weight(&self) -> f64 {
        match self {
            Self::Native => 1.0,
            Self::Learned => 0.9,
            Self::DiscoveredLocal => 0.7,
            Self::DiscoveredExternal => 0.5,
            Self::Generated => 0.4,
            Self::Deprecated => 0.1,
            Self::Disabled => 0.0,
            Self::Experimental => 0.3,
        }
    }
}

impl fmt::Display for CapabilityOrigin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Native => write!(f, "native"),
            Self::Learned => write!(f, "learned"),
            Self::DiscoveredLocal => write!(f, "discovered_local"),
            Self::DiscoveredExternal => write!(f, "discovered_external"),
            Self::Generated => write!(f, "generated"),
            Self::Deprecated => write!(f, "deprecated"),
            Self::Disabled => write!(f, "disabled"),
            Self::Experimental => write!(f, "experimental"),
        }
    }
}

// ── VerificationStatus ────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VerificationStatus {
    Unverified,
    Verified,
    Failed,
    InProgress,
}

impl fmt::Display for VerificationStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unverified => write!(f, "unverified"),
            Self::Verified => write!(f, "verified"),
            Self::Failed => write!(f, "failed"),
            Self::InProgress => write!(f, "in_progress"),
        }
    }
}

// ── TrustScore ────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrustScore {
    pub score: f64,
    pub origin: CapabilityOrigin,
    pub verification_status: VerificationStatus,
    pub execution_count: u64,
    pub success_rate: f64,
    pub failure_modes: Vec<String>,
    pub last_verified: Option<Timestamp>,
}

impl TrustScore {
    pub fn new(origin: CapabilityOrigin) -> Self {
        Self {
            score: origin.trust_weight(),
            origin,
            verification_status: VerificationStatus::Unverified,
            execution_count: 0,
            success_rate: 1.0,
            failure_modes: Vec::new(),
            last_verified: None,
        }
    }

    pub fn verified(origin: CapabilityOrigin, score: f64) -> Self {
        Self {
            score,
            origin,
            verification_status: VerificationStatus::Verified,
            execution_count: 0,
            success_rate: 1.0,
            failure_modes: Vec::new(),
            last_verified: Some(Timestamp::now()),
        }
    }
}

impl Default for TrustScore {
    fn default() -> Self {
        Self::new(CapabilityOrigin::Native)
    }
}

// ── ProviderMetadata ─────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderMetadata {
    pub name: String,
    pub version: Option<String>,
    pub provider_type: String,
    pub source: String,
    pub installed: bool,
    pub available: bool,
}

impl ProviderMetadata {
    pub fn new(
        name: impl Into<String>,
        provider_type: impl Into<String>,
        source: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            version: None,
            provider_type: provider_type.into(),
            source: source.into(),
            installed: false,
            available: true,
        }
    }
}

// ── AdapterType ──────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AdapterType {
    CliWrapper,
    RestClient,
    SdkWrapper,
    PythonScript,
    RustModule,
    PowerShell,
    BashScript,
    AppleScript,
    JavaScript,
    Container,
    Wasm,
}

impl AdapterType {
    pub fn requires_runtime(&self) -> Option<&'static str> {
        match self {
            Self::PythonScript => Some("python3"),
            Self::RustModule => Some("rustc"),
            Self::PowerShell => Some("pwsh"),
            Self::BashScript => Some("bash"),
            Self::AppleScript => Some("osascript"),
            Self::JavaScript => Some("node"),
            Self::CliWrapper => None,
            Self::RestClient => Some("curl"),
            Self::SdkWrapper => None,
            Self::Container => Some("docker"),
            Self::Wasm => Some("wasmtime"),
        }
    }
}

impl fmt::Display for AdapterType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CliWrapper => write!(f, "cli_wrapper"),
            Self::RestClient => write!(f, "rest_client"),
            Self::SdkWrapper => write!(f, "sdk_wrapper"),
            Self::PythonScript => write!(f, "python_script"),
            Self::RustModule => write!(f, "rust_module"),
            Self::PowerShell => write!(f, "powershell"),
            Self::BashScript => write!(f, "bash_script"),
            Self::AppleScript => write!(f, "applescript"),
            Self::JavaScript => write!(f, "javascript"),
            Self::Container => write!(f, "container"),
            Self::Wasm => write!(f, "wasm"),
        }
    }
}

// ── AdapterTemplate ──────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdapterTemplate {
    pub adapter_type: AdapterType,
    pub template: String,
    pub parameters: HashMap<String, String>,
    pub required_runtime: Option<String>,
    pub sandbox_profile: String,
}

// ── DiscoveredCapability ─────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiscoveredCapability {
    pub id: String,
    pub name: String,
    pub description: String,
    pub origin: CapabilityOrigin,
    pub provider: ProviderMetadata,
    pub trust: TrustScore,
    pub input_schema: HashMap<String, String>,
    pub output_schema: HashMap<String, String>,
    pub required_permissions: Vec<String>,
    pub supported_parameters: HashMap<String, String>,
    pub adapter_template: Option<AdapterTemplate>,
    pub compatible: bool,
}

// ── ResolutionStage ──────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ResolutionStage {
    NativeRegistry,
    LearnedCache,
    LocalDiscovery,
    ExternalDiscovery,
    CapabilitySynthesis,
    HumanApproval,
    Validation,
    Learning,
}

impl ResolutionStage {
    pub fn priority(&self) -> u8 {
        match self {
            Self::NativeRegistry => 1,
            Self::LearnedCache => 2,
            Self::LocalDiscovery => 3,
            Self::ExternalDiscovery => 4,
            Self::CapabilitySynthesis => 5,
            Self::HumanApproval => 6,
            Self::Validation => 7,
            Self::Learning => 8,
        }
    }
}

impl fmt::Display for ResolutionStage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NativeRegistry => write!(f, "native_registry"),
            Self::LearnedCache => write!(f, "learned_cache"),
            Self::LocalDiscovery => write!(f, "local_discovery"),
            Self::ExternalDiscovery => write!(f, "external_discovery"),
            Self::CapabilitySynthesis => write!(f, "capability_synthesis"),
            Self::HumanApproval => write!(f, "human_approval"),
            Self::Validation => write!(f, "validation"),
            Self::Learning => write!(f, "learning"),
        }
    }
}

// ── ResolutionReport ─────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResolutionReport {
    pub capability_id: String,
    pub requirement_id: String,
    pub stage: ResolutionStage,
    pub success: bool,
    pub origin: CapabilityOrigin,
    pub provider: Option<ProviderMetadata>,
    pub adapter: Option<AdapterTemplate>,
    pub duration_ms: u64,
    pub error: Option<String>,
}

// ── CapabilityCacheEntry ─────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilityCacheEntry {
    pub capability_id: String,
    pub origin: CapabilityOrigin,
    pub provider_metadata: ProviderMetadata,
    pub trust: TrustScore,
    pub supported_parameters: HashMap<String, String>,
    pub required_permissions: Vec<String>,
    pub adapter: Option<AdapterTemplate>,
    pub created_at: Timestamp,
    pub last_accessed: Timestamp,
    pub access_count: u64,
}

// ── LearnedCapability ────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LearnedCapability {
    pub capability_id: String,
    pub capability_name: String,
    pub origin: CapabilityOrigin,
    pub provider_metadata: ProviderMetadata,
    pub execution_count: u64,
    pub success_count: u64,
    pub failure_count: u64,
    pub avg_duration_ms: u64,
    pub trust_score: f64,
    pub failure_modes: Vec<String>,
    pub compatible: bool,
    pub last_execution: Option<Timestamp>,
    pub created_at: Timestamp,
}

// ── DiscoveryQuery ───────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiscoveryQuery {
    pub capability_id: String,
    pub capability_name: String,
    pub keywords: Vec<String>,
    pub categories: Vec<String>,
}

// ── CapabilitySynthesisRequest ───────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilitySynthesisRequest {
    pub capability_id: String,
    pub capability_name: String,
    pub description: String,
    pub provider: ProviderMetadata,
    pub input_schema: HashMap<String, String>,
    pub output_schema: HashMap<String, String>,
    pub preferred_adapter_type: Option<AdapterType>,
}

// ── StageBypassState ─────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StageBypassState {
    pub stage: ResolutionStage,
    pub engaged: bool,
    pub failure_count: u32,
    pub engaged_at: Option<Timestamp>,
    pub reason: Option<String>,
}

// ── ExecutionPolicy ────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionPolicy {
    pub retry: RetryPolicy,
    pub max_concurrency: usize,
    pub stage_bypass_threshold: u32,
    pub stage_bypass_window_secs: u64,
    pub default_isolation: IsolationLevel,
}

impl Default for ExecutionPolicy {
    fn default() -> Self {
        Self {
            retry: RetryPolicy::default(),
            max_concurrency: 10,
            stage_bypass_threshold: 5,
            stage_bypass_window_secs: 60,
            default_isolation: IsolationLevel::Process,
        }
    }
}

// ── ExecutionBypassState ───────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BypassState {
    pub stage: String,
    pub engaged: bool,
    pub failure_count: u32,
    pub engaged_at: Option<Timestamp>,
    pub reason: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execution_state_transitions() {
        assert!(!ExecutionState::Planned.is_terminal());
        assert!(ExecutionState::Completed.is_terminal());
        assert!(ExecutionState::Failed.is_terminal());
        assert!(ExecutionState::TimedOut.is_terminal());
        assert!(ExecutionState::Cancelled.is_terminal());
        assert!(ExecutionState::RolledBack.is_terminal());
        assert!(ExecutionState::Escalated.is_terminal());

        assert!(ExecutionState::Planned.is_active());
        assert!(ExecutionState::Running.is_active());
        assert!(!ExecutionState::Completed.is_active());

        assert!(ExecutionState::Running.can_cancel());
        assert!(ExecutionState::Planned.can_cancel());
        assert!(!ExecutionState::Completed.can_cancel());
    }

    #[test]
    fn test_execution_id() {
        let id = ExecutionId::new();
        let s = id.to_string();
        let parsed: ExecutionId = s.parse().unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn test_retry_policy_backoff() {
        let policy = RetryPolicy::default();
        assert_eq!(policy.compute_backoff(0), 0);
        assert_eq!(policy.compute_backoff(1), 1000);
        assert_eq!(policy.compute_backoff(2), 2000);
        assert_eq!(policy.compute_backoff(3), 4000);
        assert_eq!(policy.compute_backoff(4), 8000);

        let none = RetryPolicy::none();
        assert_eq!(none.max_retries, 0);
    }

    #[test]
    fn test_execution_priority_ordering() {
        assert!(ExecutionPriority::CRITICAL < ExecutionPriority::NORMAL);
        assert!(ExecutionPriority::NORMAL < ExecutionPriority::LOW);
        assert!(ExecutionPriority::LOW < ExecutionPriority::BACKGROUND);
    }

    #[test]
    fn test_sandbox_profile_defaults() {
        let profile = SandboxProfile::new("test");
        assert_eq!(profile.name, "test");
        assert!(!profile.network_access);
    }

    #[test]
    fn test_execution_budget_defaults() {
        let budget = ExecutionBudget::default();
        assert_eq!(budget.timeout_ms, 30_000);
        assert_eq!(budget.max_memory_bytes, 512 * 1024 * 1024);
    }

    #[test]
    fn test_execution_policy_defaults() {
        let policy = ExecutionPolicy::default();
        assert_eq!(policy.max_concurrency, 10);
        assert_eq!(policy.stage_bypass_threshold, 5);
    }
}
