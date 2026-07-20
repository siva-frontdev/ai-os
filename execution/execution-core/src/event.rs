use ai_os_core::events::Event;
use memory_core::Timestamp;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::types::*;

// ── Stage 1: Planner events ───────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPlanCreated {
    pub plan_id: ExecutionId,
    pub requirement_id: String,
    pub candidate_name: String,
    pub estimated_duration_ms: u64,
    pub timestamp: Timestamp,
}

impl Event for ExecutionPlanCreated {
    fn event_type(&self) -> &'static str {
        "execution.plan_created"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPlanRejected {
    pub requirement_id: String,
    pub reason: String,
    pub timestamp: Timestamp,
}

impl Event for ExecutionPlanRejected {
    fn event_type(&self) -> &'static str {
        "execution.plan_rejected"
    }
}

// ── Stage 2: Dispatcher events ─────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionQueued {
    pub execution_id: ExecutionId,
    pub queue_depth: usize,
    pub priority: u8,
    pub timestamp: Timestamp,
}

impl Event for ExecutionQueued {
    fn event_type(&self) -> &'static str {
        "execution.queued"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionDispatched {
    pub execution_id: ExecutionId,
    pub backend: String,
    pub sandbox_profile: String,
    pub timestamp: Timestamp,
}

impl Event for ExecutionDispatched {
    fn event_type(&self) -> &'static str {
        "execution.dispatched"
    }
}

// ── Stage 3: Runner events ─────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStarted {
    pub execution_id: ExecutionId,
    pub pid: Option<u32>,
    pub timestamp: Timestamp,
}

impl Event for ExecutionStarted {
    fn event_type(&self) -> &'static str {
        "execution.started"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionCompleted {
    pub execution_id: ExecutionId,
    pub exit_code: i32,
    pub duration_ms: u64,
    pub output_size: u64,
    pub timestamp: Timestamp,
}

impl Event for ExecutionCompleted {
    fn event_type(&self) -> &'static str {
        "execution.completed"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionFailed {
    pub execution_id: ExecutionId,
    pub exit_code: i32,
    pub error: String,
    pub partial_output: bool,
    pub timestamp: Timestamp,
}

impl Event for ExecutionFailed {
    fn event_type(&self) -> &'static str {
        "execution.failed"
    }
}

// ── Stage 4: Monitor events ────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionTimedOut {
    pub execution_id: ExecutionId,
    pub timeout_ms: u64,
    pub elapsed_ms: u64,
    pub partial_output: bool,
    pub timestamp: Timestamp,
}

impl Event for ExecutionTimedOut {
    fn event_type(&self) -> &'static str {
        "execution.timed_out"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimitExceeded {
    pub execution_id: ExecutionId,
    pub resource: String,
    pub used: u64,
    pub max: u64,
    pub timestamp: Timestamp,
}

impl Event for ResourceLimitExceeded {
    fn event_type(&self) -> &'static str {
        "execution.resource_exceeded"
    }
}

// ── Result events ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResultRouted {
    pub execution_id: ExecutionId,
    pub destination: String,
    pub success: bool,
    pub timestamp: Timestamp,
}

impl Event for ExecutionResultRouted {
    fn event_type(&self) -> &'static str {
        "execution.result_routed"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResultRouteFailed {
    pub execution_id: ExecutionId,
    pub destination: String,
    pub error: String,
    pub timestamp: Timestamp,
}

impl Event for ExecutionResultRouteFailed {
    fn event_type(&self) -> &'static str {
        "execution.result_route_failed"
    }
}

// ── Recovery events ────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionRetrying {
    pub execution_id: ExecutionId,
    pub attempt: u32,
    pub max_retries: u32,
    pub backoff_ms: u64,
    pub reason: String,
    pub timestamp: Timestamp,
}

impl Event for ExecutionRetrying {
    fn event_type(&self) -> &'static str {
        "execution.retrying"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionRollingBack {
    pub execution_id: ExecutionId,
    pub steps: Vec<String>,
    pub timestamp: Timestamp,
}

impl Event for ExecutionRollingBack {
    fn event_type(&self) -> &'static str {
        "execution.rolling_back"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionRolledBack {
    pub execution_id: ExecutionId,
    pub steps_completed: u32,
    pub steps_failed: u32,
    pub timestamp: Timestamp,
}

impl Event for ExecutionRolledBack {
    fn event_type(&self) -> &'static str {
        "execution.rolled_back"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionCancelled {
    pub execution_id: ExecutionId,
    pub reason: String,
    pub partial_output: bool,
    pub timestamp: Timestamp,
}

impl Event for ExecutionCancelled {
    fn event_type(&self) -> &'static str {
        "execution.cancelled"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionEscalated {
    pub execution_id: ExecutionId,
    pub reason: String,
    pub context: String,
    pub timestamp: Timestamp,
}

impl Event for ExecutionEscalated {
    fn event_type(&self) -> &'static str {
        "execution.escalated"
    }
}

// ── Registry events ────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolRegistered {
    pub tool_id: String,
    pub name: String,
    pub capabilities: Vec<String>,
    pub timestamp: Timestamp,
}

impl Event for ToolRegistered {
    fn event_type(&self) -> &'static str {
        "execution.tool_registered"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolUnregistered {
    pub tool_id: String,
    pub timestamp: Timestamp,
}

impl Event for ToolUnregistered {
    fn event_type(&self) -> &'static str {
        "execution.tool_unregistered"
    }
}

// ── Pipeline events ────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackpressureWarning {
    pub stage: String,
    pub capacity: usize,
    pub usage: usize,
    pub timestamp: Timestamp,
}

impl Event for BackpressureWarning {
    fn event_type(&self) -> &'static str {
        "execution.backpressure_warning"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BypassEngaged {
    pub stage: String,
    pub reason: String,
    pub timestamp: Timestamp,
}

impl Event for BypassEngaged {
    fn event_type(&self) -> &'static str {
        "execution.bypass_engaged"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BypassDisengaged {
    pub stage: String,
    pub timestamp: Timestamp,
}

impl Event for BypassDisengaged {
    fn event_type(&self) -> &'static str {
        "execution.bypass_disengaged"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoordinatorReady {
    pub stages: Vec<String>,
    pub timestamp: Timestamp,
}

impl Event for CoordinatorReady {
    fn event_type(&self) -> &'static str {
        "execution.coordinator_ready"
    }
}

// ── Enum dispatch ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event_type", content = "payload")]
pub enum ExecutionEvent {
    #[serde(rename = "execution.plan_created")]
    PlanCreated(ExecutionPlanCreated),
    #[serde(rename = "execution.plan_rejected")]
    PlanRejected(ExecutionPlanRejected),
    #[serde(rename = "execution.queued")]
    Queued(ExecutionQueued),
    #[serde(rename = "execution.dispatched")]
    Dispatched(ExecutionDispatched),
    #[serde(rename = "execution.started")]
    Started(ExecutionStarted),
    #[serde(rename = "execution.completed")]
    Completed(ExecutionCompleted),
    #[serde(rename = "execution.failed")]
    Failed(ExecutionFailed),
    #[serde(rename = "execution.timed_out")]
    TimedOut(ExecutionTimedOut),
    #[serde(rename = "execution.resource_exceeded")]
    ResourceExceeded(ResourceLimitExceeded),
    #[serde(rename = "execution.result_routed")]
    ResultRouted(ExecutionResultRouted),
    #[serde(rename = "execution.result_route_failed")]
    ResultRouteFailed(ExecutionResultRouteFailed),
    #[serde(rename = "execution.retrying")]
    Retrying(ExecutionRetrying),
    #[serde(rename = "execution.rolling_back")]
    RollingBack(ExecutionRollingBack),
    #[serde(rename = "execution.rolled_back")]
    RolledBack(ExecutionRolledBack),
    #[serde(rename = "execution.cancelled")]
    Cancelled(ExecutionCancelled),
    #[serde(rename = "execution.escalated")]
    Escalated(ExecutionEscalated),
    #[serde(rename = "execution.tool_registered")]
    ToolRegistered(ToolRegistered),
    #[serde(rename = "execution.tool_unregistered")]
    ToolUnregistered(ToolUnregistered),
    #[serde(rename = "execution.backpressure_warning")]
    BackpressureWarning(BackpressureWarning),
    #[serde(rename = "execution.bypass_engaged")]
    BypassEngaged(BypassEngaged),
    #[serde(rename = "execution.bypass_disengaged")]
    BypassDisengaged(BypassDisengaged),
    #[serde(rename = "execution.coordinator_ready")]
    CoordinatorReady(CoordinatorReady),
}

impl ExecutionEvent {
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::PlanCreated(_) => "execution.plan_created",
            Self::PlanRejected(_) => "execution.plan_rejected",
            Self::Queued(_) => "execution.queued",
            Self::Dispatched(_) => "execution.dispatched",
            Self::Started(_) => "execution.started",
            Self::Completed(_) => "execution.completed",
            Self::Failed(_) => "execution.failed",
            Self::TimedOut(_) => "execution.timed_out",
            Self::ResourceExceeded(_) => "execution.resource_exceeded",
            Self::ResultRouted(_) => "execution.result_routed",
            Self::ResultRouteFailed(_) => "execution.result_route_failed",
            Self::Retrying(_) => "execution.retrying",
            Self::RollingBack(_) => "execution.rolling_back",
            Self::RolledBack(_) => "execution.rolled_back",
            Self::Cancelled(_) => "execution.cancelled",
            Self::Escalated(_) => "execution.escalated",
            Self::ToolRegistered(_) => "execution.tool_registered",
            Self::ToolUnregistered(_) => "execution.tool_unregistered",
            Self::BackpressureWarning(_) => "execution.backpressure_warning",
            Self::BypassEngaged(_) => "execution.bypass_engaged",
            Self::BypassDisengaged(_) => "execution.bypass_disengaged",
            Self::CoordinatorReady(_) => "execution.coordinator_ready",
        }
    }

    pub fn metadata(&self) -> HashMap<String, String> {
        let mut m = HashMap::new();
        m.insert("event_type".into(), self.event_type().into());
        match self {
            Self::PlanCreated(e) => {
                m.insert("plan_id".into(), e.plan_id.to_string());
            }
            Self::PlanRejected(e) => {
                m.insert("reason".into(), e.reason.clone());
            }
            Self::Queued(e) => {
                m.insert("execution_id".into(), e.execution_id.to_string());
            }
            Self::Dispatched(e) => {
                m.insert("execution_id".into(), e.execution_id.to_string());
                m.insert("backend".into(), e.backend.clone());
            }
            Self::Started(e) => {
                m.insert("execution_id".into(), e.execution_id.to_string());
            }
            Self::Completed(e) => {
                m.insert("execution_id".into(), e.execution_id.to_string());
                m.insert("exit_code".into(), e.exit_code.to_string());
            }
            Self::Failed(e) => {
                m.insert("execution_id".into(), e.execution_id.to_string());
                m.insert("exit_code".into(), e.exit_code.to_string());
            }
            Self::TimedOut(e) => {
                m.insert("execution_id".into(), e.execution_id.to_string());
            }
            Self::ResourceExceeded(e) => {
                m.insert("execution_id".into(), e.execution_id.to_string());
                m.insert("resource".into(), e.resource.clone());
            }
            Self::ResultRouted(e) => {
                m.insert("execution_id".into(), e.execution_id.to_string());
                m.insert("destination".into(), e.destination.clone());
            }
            Self::ResultRouteFailed(e) => {
                m.insert("execution_id".into(), e.execution_id.to_string());
                m.insert("destination".into(), e.destination.clone());
            }
            Self::Retrying(e) => {
                m.insert("execution_id".into(), e.execution_id.to_string());
                m.insert("attempt".into(), e.attempt.to_string());
            }
            Self::RollingBack(e) => {
                m.insert("execution_id".into(), e.execution_id.to_string());
            }
            Self::RolledBack(e) => {
                m.insert("execution_id".into(), e.execution_id.to_string());
            }
            Self::Cancelled(e) => {
                m.insert("execution_id".into(), e.execution_id.to_string());
                m.insert("reason".into(), e.reason.clone());
            }
            Self::Escalated(e) => {
                m.insert("execution_id".into(), e.execution_id.to_string());
                m.insert("reason".into(), e.reason.clone());
            }
            Self::ToolRegistered(e) => {
                m.insert("tool_id".into(), e.tool_id.clone());
            }
            Self::ToolUnregistered(e) => {
                m.insert("tool_id".into(), e.tool_id.clone());
            }
            Self::BackpressureWarning(e) => {
                m.insert("stage".into(), e.stage.clone());
            }
            Self::BypassEngaged(e) => {
                m.insert("stage".into(), e.stage.clone());
            }
            Self::BypassDisengaged(e) => {
                m.insert("stage".into(), e.stage.clone());
            }
            Self::CoordinatorReady(_) => {}
        }
        m
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_type_strings() {
        let ts = Timestamp::now();
        let id = ExecutionId::new();
        let eid = ExecutionId::new();

        let events: Vec<ExecutionEvent> = vec![
            ExecutionEvent::PlanCreated(ExecutionPlanCreated {
                plan_id: id,
                requirement_id: "r1".into(),
                candidate_name: "grep".into(),
                estimated_duration_ms: 100,
                timestamp: ts,
            }),
            ExecutionEvent::PlanRejected(ExecutionPlanRejected {
                requirement_id: "r1".into(),
                reason: "no candidate".into(),
                timestamp: ts,
            }),
            ExecutionEvent::Queued(ExecutionQueued {
                execution_id: id,
                queue_depth: 5,
                priority: 50,
                timestamp: ts,
            }),
            ExecutionEvent::Dispatched(ExecutionDispatched {
                execution_id: id,
                backend: "subprocess".into(),
                sandbox_profile: "default".into(),
                timestamp: ts,
            }),
            ExecutionEvent::Started(ExecutionStarted {
                execution_id: id,
                pid: Some(1234),
                timestamp: ts,
            }),
            ExecutionEvent::Completed(ExecutionCompleted {
                execution_id: id,
                exit_code: 0,
                duration_ms: 50,
                output_size: 100,
                timestamp: ts,
            }),
            ExecutionEvent::Failed(ExecutionFailed {
                execution_id: id,
                exit_code: 1,
                error: "oops".into(),
                partial_output: true,
                timestamp: ts,
            }),
            ExecutionEvent::TimedOut(ExecutionTimedOut {
                execution_id: id,
                timeout_ms: 1000,
                elapsed_ms: 1000,
                partial_output: true,
                timestamp: ts,
            }),
            ExecutionEvent::ResourceExceeded(ResourceLimitExceeded {
                execution_id: id,
                resource: "memory".into(),
                used: 600,
                max: 512,
                timestamp: ts,
            }),
            ExecutionEvent::ResultRouted(ExecutionResultRouted {
                execution_id: id,
                destination: "caller".into(),
                success: true,
                timestamp: ts,
            }),
            ExecutionEvent::ResultRouteFailed(ExecutionResultRouteFailed {
                execution_id: id,
                destination: "memory".into(),
                error: "store full".into(),
                timestamp: ts,
            }),
            ExecutionEvent::Retrying(ExecutionRetrying {
                execution_id: id,
                attempt: 1,
                max_retries: 3,
                backoff_ms: 1000,
                reason: "exit 1".into(),
                timestamp: ts,
            }),
            ExecutionEvent::RollingBack(ExecutionRollingBack {
                execution_id: id,
                steps: vec!["restore_snapshot".into()],
                timestamp: ts,
            }),
            ExecutionEvent::RolledBack(ExecutionRolledBack {
                execution_id: id,
                steps_completed: 1,
                steps_failed: 0,
                timestamp: ts,
            }),
            ExecutionEvent::Cancelled(ExecutionCancelled {
                execution_id: id,
                reason: "user request".into(),
                partial_output: true,
                timestamp: ts,
            }),
            ExecutionEvent::Escalated(ExecutionEscalated {
                execution_id: id,
                reason: "retry exhausted".into(),
                context: "{}".into(),
                timestamp: ts,
            }),
            ExecutionEvent::ToolRegistered(ToolRegistered {
                tool_id: "t1".into(),
                name: "grep".into(),
                capabilities: vec!["text.search".into()],
                timestamp: ts,
            }),
            ExecutionEvent::ToolUnregistered(ToolUnregistered {
                tool_id: "t1".into(),
                timestamp: ts,
            }),
            ExecutionEvent::BackpressureWarning(BackpressureWarning {
                stage: "dispatcher".into(),
                capacity: 10000,
                usage: 9000,
                timestamp: ts,
            }),
            ExecutionEvent::BypassEngaged(BypassEngaged {
                stage: "planner".into(),
                reason: "5 failures".into(),
                timestamp: ts,
            }),
            ExecutionEvent::BypassDisengaged(BypassDisengaged {
                stage: "planner".into(),
                timestamp: ts,
            }),
            ExecutionEvent::CoordinatorReady(CoordinatorReady {
                stages: vec!["planner".into(), "dispatcher".into()],
                timestamp: ts,
            }),
        ];

        let expected = [
            "execution.plan_created",
            "execution.plan_rejected",
            "execution.queued",
            "execution.dispatched",
            "execution.started",
            "execution.completed",
            "execution.failed",
            "execution.timed_out",
            "execution.resource_exceeded",
            "execution.result_routed",
            "execution.result_route_failed",
            "execution.retrying",
            "execution.rolling_back",
            "execution.rolled_back",
            "execution.cancelled",
            "execution.escalated",
            "execution.tool_registered",
            "execution.tool_unregistered",
            "execution.backpressure_warning",
            "execution.bypass_engaged",
            "execution.bypass_disengaged",
            "execution.coordinator_ready",
        ];

        for (event, expected_type) in events.iter().zip(expected.iter()) {
            assert_eq!(
                event.event_type(),
                *expected_type,
                "mismatch for {:?}",
                event
            );
        }
    }

    #[test]
    fn test_event_metadata() {
        let event = ExecutionEvent::Started(ExecutionStarted {
            execution_id: ExecutionId::new(),
            pid: Some(99),
            timestamp: Timestamp::now(),
        });
        let meta = event.metadata();
        assert_eq!(meta.get("event_type").unwrap(), "execution.started");
        assert!(meta.contains_key("execution_id"));
    }
}
