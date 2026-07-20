//! Tool contracts: `ToolRequirement`, `ToolCandidate`, `ExecutablePlan`, `ToolRegistry`.
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::ids::{GoalId, PlanId, ToolCapabilityId, ToolId};
use crate::types::Confidence;

// ── Parameter types ───────────────────────────────────────────

/// Type of a tool parameter value.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ToolParameterType {
    String,
    Number,
    Boolean,
    Array,
    Object,
    FilePath,
    Duration,
}

/// A single parameter in a tool's I/O schema.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ToolParameter {
    pub name: String,
    pub param_type: ToolParameterType,
    pub required: bool,
    pub description: String,
}

impl ToolParameter {
    pub fn new<N, D>(name: N, param_type: ToolParameterType, description: D) -> Self
    where
        N: Into<String>,
        D: Into<String>,
    {
        Self {
            name: name.into(),
            param_type,
            required: true,
            description: description.into(),
        }
    }
    pub fn optional(mut self) -> Self {
        self.required = false;
        self
    }
}

// ── Capability ─────────────────────────────────────────────────

/// A declared capability a tool may provide.
///
/// Stored in the `ToolRegistry` to allow lookup by `ToolCapabilityId`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ToolCapability {
    pub id: ToolCapabilityId,
    pub name: String,
    pub description: String,
    pub input_schema: Vec<ToolParameter>,
    pub output_schema: Option<Vec<ToolParameter>>,
}

impl ToolCapability {
    pub fn new<N, D>(id: ToolCapabilityId, name: N, description: D) -> Self
    where
        N: Into<String>,
        D: Into<String>,
    {
        Self {
            id,
            name: name.into(),
            description: description.into(),
            input_schema: Vec::new(),
            output_schema: None,
        }
    }
}

// ── Tool value ─────────────────────────────────────────────────

/// An abstract input value (not an OS handle).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ToolValue {
    String(String),
    Number(f64),
    Bool(bool),
    List(Vec<ToolValue>),
    Map(HashMap<String, ToolValue>),
    FilePath(String),
    DurationMs(u64),
    Null,
}

impl ToolValue {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s),
            _ => None,
        }
    }
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Number(n) => Some(*n),
            _ => None,
        }
    }
}

impl Default for ToolValue {
    fn default() -> Self {
        Self::Null
    }
}

// ── Side effects ──────────────────────────────────────────────

/// Declared side effect of a tool candidate.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SideEffect {
    FilesystemWrite(String),
    NetworkRequest(String),
    ProcessSpawn,
    StateMutation(String),
    NoSideEffects,
}

impl std::fmt::Display for SideEffect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FilesystemWrite(p) => write!(f, "FilesystemWrite({})", p),
            Self::NetworkRequest(h) => write!(f, "NetworkRequest({})", h),
            Self::ProcessSpawn => write!(f, "ProcessSpawn"),
            Self::StateMutation(d) => write!(f, "StateMutation({})", d),
            Self::NoSideEffects => write!(f, "NoSideEffects"),
        }
    }
}

// ── Retry policy ──────────────────────────────────────────────

/// Retry behaviour for a tool requirement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetryPolicy {
    pub max_retries: u32,
    pub initial_delay_ms: u64,
}

impl RetryPolicy {
    pub fn new(max_retries: u32, initial_delay_ms: u64) -> Self {
        Self {
            max_retries,
            initial_delay_ms,
        }
    }
    pub fn none() -> Self {
        Self {
            max_retries: 0,
            initial_delay_ms: 0,
        }
    }
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self::new(1, 500)
    }
}

// ── ToolRequirement ─────────────────────────────────────────

/// Abstract declaration of a capability the planner needs.
///
/// A `ToolRequirement` declares **what** is needed, not **which tool**
/// provides it. `ToolSelector` matches this to one or more `ToolCandidate`s.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolRequirement {
    pub requirement_id: uuid::Uuid,
    pub capability: ToolCapabilityId,
    pub inputs: HashMap<String, ToolValue>,
    pub expected_outputs: Vec<String>,
    pub timeout_ms: Option<u64>,
    pub retry_policy: RetryPolicy,
    pub priority: u8,
}

impl ToolRequirement {
    pub fn new(
        capability: ToolCapabilityId,
        inputs: HashMap<String, ToolValue>,
        expected_outputs: Vec<String>,
    ) -> Self {
        Self {
            requirement_id: uuid::Uuid::new_v4(),
            capability,
            inputs,
            expected_outputs,
            timeout_ms: None,
            retry_policy: RetryPolicy::default(),
            priority: 128,
        }
    }
}

// ── ToolCandidate ─────────────────────────────────────────────

/// A concrete tool that can satisfy a [`ToolRequirement`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCandidate {
    pub tool_id: ToolId,
    pub provider: String,
    pub capability: ToolCapabilityId,
    pub estimated_cost: f64,
    pub estimated_duration_ms: u64,
    pub confidence: Confidence,
    pub side_effects: Vec<SideEffect>,
}

impl ToolCandidate {
    pub fn new(
        tool_id: ToolId,
        provider: impl Into<String>,
        capability: ToolCapabilityId,
        confidence: Confidence,
    ) -> Self {
        Self {
            tool_id,
            provider: provider.into(),
            capability,
            estimated_cost: 0.0,
            estimated_duration_ms: 0,
            confidence,
            side_effects: vec![SideEffect::NoSideEffects],
        }
    }
}

// ── ExecutablePlan (metadata; no execution logic) ─────────────

/// A plan produced by the Planner, ready for policy evaluation.
///
/// Contains abstract `ToolRequirement` objects, not OS commands.
/// Execution Platform (Phase 8) maps these to actual dispatches.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutablePlan {
    pub plan_id: PlanId,
    pub goal_id: GoalId,
    pub tool_requirements: Vec<ToolRequirement>,
    pub fallback_requirements: Vec<Vec<ToolRequirement>>,
    pub estimated_cost: f64,
    pub estimated_duration_ms: u64,
    pub status: PlanStatus,
}

impl ExecutablePlan {
    pub fn new(plan_id: PlanId, goal_id: GoalId, tool_requirements: Vec<ToolRequirement>) -> Self {
        Self {
            plan_id,
            goal_id,
            tool_requirements,
            fallback_requirements: Vec::new(),
            estimated_cost: 0.0,
            estimated_duration_ms: 0,
            status: PlanStatus::Draft,
        }
    }
}

/// Status of a plan in its lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlanStatus {
    Draft,
    Approved,
    Rejected,
    Executing,
    Completed,
    Failed,
}

impl std::fmt::Display for PlanStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Draft => f.write_str("Draft"),
            Self::Approved => f.write_str("Approved"),
            Self::Rejected => f.write_str("Rejected"),
            Self::Executing => f.write_str("Executing"),
            Self::Completed => f.write_str("Completed"),
            Self::Failed => f.write_str("Failed"),
        }
    }
}

// ── ToolRegistry trait ────────────────────────────────────────

/// Trait for looking up tool candidates by capability.
#[async_trait::async_trait]
pub trait ToolRegistry: Send + Sync {
    async fn find_candidates(
        &self,
        capability: ToolCapabilityId,
    ) -> crate::BrainResult<Vec<ToolCandidate>>;
    async fn register(&self, candidate: ToolCandidate) -> crate::BrainResult<()>;
    async fn deregister(&self, tool_id: ToolId) -> crate::BrainResult<()>;
    async fn list_capabilities(&self) -> crate::BrainResult<Vec<ToolCapability>>;
}
