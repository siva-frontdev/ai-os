use ai_os_core::events::Event;
use memory_core::Timestamp;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::types::*;

// ── Stage 1: Observer events ──────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservationReceived {
    pub observation_id: ObservationId,
    pub source: ObservationSource,
    pub modality: Modality,
    pub size_bytes: u64,
    pub timestamp: Timestamp,
}

impl Event for ObservationReceived {
    fn event_type(&self) -> &'static str {
        "perception.observation.received"
    }
}

// ── Stage 2: Normalizer events ────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservationNormalized {
    pub observation_id: ObservationId,
    pub format: String,
    pub schema_version: Option<String>,
    pub timestamp: Timestamp,
}

impl Event for ObservationNormalized {
    fn event_type(&self) -> &'static str {
        "perception.observation.normalized"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservationRejected {
    pub observation_id: ObservationId,
    pub reason: String,
    pub timestamp: Timestamp,
}

impl Event for ObservationRejected {
    fn event_type(&self) -> &'static str {
        "perception.observation.rejected"
    }
}

// ── Stage 3: Attention Filter events ──────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservationFiltered {
    pub observation_id: ObservationId,
    pub score: f64,
    pub threshold: f64,
    pub reason: String,
    pub timestamp: Timestamp,
}

impl Event for ObservationFiltered {
    fn event_type(&self) -> &'static str {
        "perception.observation.filtered"
    }
}

// ── Stage 4: Entity Extractor events ──────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntitiesExtracted {
    pub observation_id: ObservationId,
    pub entity_count: u32,
    pub timestamp: Timestamp,
}

impl Event for EntitiesExtracted {
    fn event_type(&self) -> &'static str {
        "perception.entities.extracted"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityResolutionFailed {
    pub entity_type: String,
    pub raw_value: String,
    pub reason: String,
    pub timestamp: Timestamp,
}

impl Event for EntityResolutionFailed {
    fn event_type(&self) -> &'static str {
        "perception.entities.resolution_failed"
    }
}

// ── Stage 5: Context Enricher events ──────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextEnriched {
    pub observation_id: ObservationId,
    pub session_id: Option<String>,
    pub timestamp: Timestamp,
}

impl Event for ContextEnriched {
    fn event_type(&self) -> &'static str {
        "perception.context.enriched"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionResolutionFailed {
    pub session_id: Option<String>,
    pub reason: String,
    pub timestamp: Timestamp,
}

impl Event for SessionResolutionFailed {
    fn event_type(&self) -> &'static str {
        "perception.context.session_failed"
    }
}

// ── Stage 6: State Detector events ────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateChanged {
    pub scope: ScopeId,
    pub from: String,
    pub to: String,
    pub observation_id: ObservationId,
    pub timestamp: Timestamp,
}

impl Event for StateChanged {
    fn event_type(&self) -> &'static str {
        "perception.state.changed"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateMachineConflict {
    pub scope: ScopeId,
    pub from: String,
    pub to: String,
    pub reason: String,
    pub timestamp: Timestamp,
}

impl Event for StateMachineConflict {
    fn event_type(&self) -> &'static str {
        "perception.state.conflict"
    }
}

// ── Stage 7: Anomaly Detector events ──────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyDetected {
    pub observation_id: ObservationId,
    pub score: f64,
    pub threshold: f64,
    pub model: String,
    pub timestamp: Timestamp,
}

impl Event for AnomalyDetected {
    fn event_type(&self) -> &'static str {
        "perception.anomaly.detected"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyBaselineUpdated {
    pub modality: Modality,
    pub window_size: usize,
    pub sample_count: u64,
    pub timestamp: Timestamp,
}

impl Event for AnomalyBaselineUpdated {
    fn event_type(&self) -> &'static str {
        "perception.anomaly.baseline"
    }
}

// ── Stage 8: Fusion Engine events ─────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservationFused {
    pub correlation_key: CorrelationKey,
    pub sources: Vec<ObservationSource>,
    pub confidence: f64,
    pub constituent_ids: Vec<ObservationId>,
    pub timestamp: Timestamp,
}

impl Event for ObservationFused {
    fn event_type(&self) -> &'static str {
        "perception.fusion.complete"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FusionWindowExpired {
    pub correlation_key: CorrelationKey,
    pub expected_sources: Vec<String>,
    pub received: Vec<String>,
    pub timestamp: Timestamp,
}

impl Event for FusionWindowExpired {
    fn event_type(&self) -> &'static str {
        "perception.fusion.window_expired"
    }
}

// ── Coordinator / Lifecycle events ────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservationReady {
    pub observation_id: ObservationId,
    pub modality: Modality,
    pub priority: ObservationPriority,
    pub timestamp: Timestamp,
}

impl Event for ObservationReady {
    fn event_type(&self) -> &'static str {
        "perception.observation.ready"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineStageFailed {
    pub stage: String,
    pub error: String,
    pub restarts: u32,
    pub timestamp: Timestamp,
}

impl Event for PipelineStageFailed {
    fn event_type(&self) -> &'static str {
        "perception.pipeline.stage_failed"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineBackpressure {
    pub stage: String,
    pub channel_capacity: usize,
    pub drop_count: u64,
    pub timestamp: Timestamp,
}

impl Event for PipelineBackpressure {
    fn event_type(&self) -> &'static str {
        "perception.pipeline.backpressure"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineReconfigured {
    pub sections_changed: Vec<String>,
    pub timestamp: Timestamp,
}

impl Event for PipelineReconfigured {
    fn event_type(&self) -> &'static str {
        "perception.pipeline.reconfigured"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObserverStatusChanged {
    pub observer_id: String,
    pub old_status: String,
    pub new_status: String,
    pub timestamp: Timestamp,
}

impl Event for ObserverStatusChanged {
    fn event_type(&self) -> &'static str {
        "perception.observer.status"
    }
}

// ── Enum dispatch for all perception events ───────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event_type", content = "payload")]
pub enum PerceptionEvent {
    #[serde(rename = "perception.observation.received")]
    ObservationReceived(ObservationReceived),
    #[serde(rename = "perception.observation.normalized")]
    ObservationNormalized(ObservationNormalized),
    #[serde(rename = "perception.observation.rejected")]
    ObservationRejected(ObservationRejected),
    #[serde(rename = "perception.observation.filtered")]
    ObservationFiltered(ObservationFiltered),
    #[serde(rename = "perception.entities.extracted")]
    EntitiesExtracted(EntitiesExtracted),
    #[serde(rename = "perception.entities.resolution_failed")]
    EntityResolutionFailed(EntityResolutionFailed),
    #[serde(rename = "perception.context.enriched")]
    ContextEnriched(ContextEnriched),
    #[serde(rename = "perception.context.session_failed")]
    SessionResolutionFailed(SessionResolutionFailed),
    #[serde(rename = "perception.state.changed")]
    StateChanged(StateChanged),
    #[serde(rename = "perception.state.conflict")]
    StateMachineConflict(StateMachineConflict),
    #[serde(rename = "perception.anomaly.detected")]
    AnomalyDetected(AnomalyDetected),
    #[serde(rename = "perception.anomaly.baseline")]
    AnomalyBaselineUpdated(AnomalyBaselineUpdated),
    #[serde(rename = "perception.fusion.complete")]
    ObservationFused(ObservationFused),
    #[serde(rename = "perception.fusion.window_expired")]
    FusionWindowExpired(FusionWindowExpired),
    #[serde(rename = "perception.observation.ready")]
    ObservationReady(ObservationReady),
    #[serde(rename = "perception.pipeline.stage_failed")]
    PipelineStageFailed(PipelineStageFailed),
    #[serde(rename = "perception.pipeline.backpressure")]
    PipelineBackpressure(PipelineBackpressure),
    #[serde(rename = "perception.pipeline.reconfigured")]
    PipelineReconfigured(PipelineReconfigured),
    #[serde(rename = "perception.observer.status")]
    ObserverStatusChanged(ObserverStatusChanged),
}

impl PerceptionEvent {
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::ObservationReceived(_) => "perception.observation.received",
            Self::ObservationNormalized(_) => "perception.observation.normalized",
            Self::ObservationRejected(_) => "perception.observation.rejected",
            Self::ObservationFiltered(_) => "perception.observation.filtered",
            Self::EntitiesExtracted(_) => "perception.entities.extracted",
            Self::EntityResolutionFailed(_) => "perception.entities.resolution_failed",
            Self::ContextEnriched(_) => "perception.context.enriched",
            Self::SessionResolutionFailed(_) => "perception.context.session_failed",
            Self::StateChanged(_) => "perception.state.changed",
            Self::StateMachineConflict(_) => "perception.state.conflict",
            Self::AnomalyDetected(_) => "perception.anomaly.detected",
            Self::AnomalyBaselineUpdated(_) => "perception.anomaly.baseline",
            Self::ObservationFused(_) => "perception.fusion.complete",
            Self::FusionWindowExpired(_) => "perception.fusion.window_expired",
            Self::ObservationReady(_) => "perception.observation.ready",
            Self::PipelineStageFailed(_) => "perception.pipeline.stage_failed",
            Self::PipelineBackpressure(_) => "perception.pipeline.backpressure",
            Self::PipelineReconfigured(_) => "perception.pipeline.reconfigured",
            Self::ObserverStatusChanged(_) => "perception.observer.status",
        }
    }

    pub fn metadata(&self) -> HashMap<String, String> {
        let mut m = HashMap::new();
        m.insert("event_type".into(), self.event_type().into());
        match self {
            Self::ObservationReceived(e) => {
                m.insert("observation_id".into(), e.observation_id.to_string());
                m.insert("modality".into(), format!("{:?}", e.modality));
            }
            Self::ObservationNormalized(e) => {
                m.insert("observation_id".into(), e.observation_id.to_string());
                m.insert("format".into(), e.format.clone());
            }
            Self::ObservationRejected(e) => {
                m.insert("observation_id".into(), e.observation_id.to_string());
            }
            Self::ObservationFiltered(e) => {
                m.insert("observation_id".into(), e.observation_id.to_string());
            }
            Self::EntitiesExtracted(e) => {
                m.insert("observation_id".into(), e.observation_id.to_string());
                m.insert("entity_count".into(), e.entity_count.to_string());
            }
            Self::EntityResolutionFailed(e) => {
                m.insert("entity_type".into(), e.entity_type.clone());
            }
            Self::ContextEnriched(e) => {
                m.insert("observation_id".into(), e.observation_id.to_string());
            }
            Self::SessionResolutionFailed(e) => {
                m.insert("reason".into(), e.reason.clone());
            }
            Self::StateChanged(e) => {
                m.insert("scope".into(), e.scope.to_string());
                m.insert("from".into(), e.from.clone());
                m.insert("to".into(), e.to.clone());
            }
            Self::StateMachineConflict(e) => {
                m.insert("scope".into(), e.scope.to_string());
            }
            Self::AnomalyDetected(e) => {
                m.insert("observation_id".into(), e.observation_id.to_string());
                m.insert("score".into(), e.score.to_string());
            }
            Self::AnomalyBaselineUpdated(e) => {
                m.insert("modality".into(), format!("{:?}", e.modality));
            }
            Self::ObservationFused(e) => {
                m.insert("correlation_key".into(), e.correlation_key.to_string());
            }
            Self::FusionWindowExpired(e) => {
                m.insert("correlation_key".into(), e.correlation_key.to_string());
            }
            Self::ObservationReady(e) => {
                m.insert("observation_id".into(), e.observation_id.to_string());
            }
            Self::PipelineStageFailed(e) => {
                m.insert("stage".into(), e.stage.clone());
            }
            Self::PipelineBackpressure(e) => {
                m.insert("stage".into(), e.stage.clone());
            }
            Self::PipelineReconfigured(_) => {}
            Self::ObserverStatusChanged(e) => {
                m.insert("observer_id".into(), e.observer_id.clone());
            }
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
        let id = ObservationId::new();
        let source = ObservationSource {
            observer_id: "test".into(),
            observer_kind: ObserverKind::FileSystem,
            instance_id: "inst-1".into(),
            hostname: "host".into(),
        };
        let obs_id = ObservationId::new();
        let scope = ScopeId("test-scope".into());
        let corr_key = CorrelationKey("test-key".into());

        let events: Vec<PerceptionEvent> = vec![
            PerceptionEvent::ObservationReceived(ObservationReceived {
                observation_id: id,
                source: source.clone(),
                modality: Modality::FileSystem,
                size_bytes: 100,
                timestamp: ts,
            }),
            PerceptionEvent::ObservationNormalized(ObservationNormalized {
                observation_id: id,
                format: "json".into(),
                schema_version: None,
                timestamp: ts,
            }),
            PerceptionEvent::ObservationRejected(ObservationRejected {
                observation_id: id,
                reason: "invalid schema".into(),
                timestamp: ts,
            }),
            PerceptionEvent::ObservationFiltered(ObservationFiltered {
                observation_id: id,
                score: 0.1,
                threshold: 0.5,
                reason: "low score".into(),
                timestamp: ts,
            }),
            PerceptionEvent::EntitiesExtracted(EntitiesExtracted {
                observation_id: id,
                entity_count: 3,
                timestamp: ts,
            }),
            PerceptionEvent::EntityResolutionFailed(EntityResolutionFailed {
                entity_type: "process".into(),
                raw_value: "1234".into(),
                reason: "timeout".into(),
                timestamp: ts,
            }),
            PerceptionEvent::ContextEnriched(ContextEnriched {
                observation_id: id,
                session_id: Some("sess-1".into()),
                timestamp: ts,
            }),
            PerceptionEvent::SessionResolutionFailed(SessionResolutionFailed {
                session_id: None,
                reason: "not found".into(),
                timestamp: ts,
            }),
            PerceptionEvent::StateChanged(StateChanged {
                scope: scope.clone(),
                from: "idle".into(),
                to: "active".into(),
                observation_id: id,
                timestamp: ts,
            }),
            PerceptionEvent::StateMachineConflict(StateMachineConflict {
                scope: scope.clone(),
                from: "active".into(),
                to: "idle".into(),
                reason: "invalid transition".into(),
                timestamp: ts,
            }),
            PerceptionEvent::AnomalyDetected(AnomalyDetected {
                observation_id: id,
                score: 0.95,
                threshold: 0.8,
                model: "zscore".into(),
                timestamp: ts,
            }),
            PerceptionEvent::AnomalyBaselineUpdated(AnomalyBaselineUpdated {
                modality: Modality::Network,
                window_size: 100,
                sample_count: 50,
                timestamp: ts,
            }),
            PerceptionEvent::ObservationFused(ObservationFused {
                correlation_key: corr_key.clone(),
                sources: vec![source.clone()],
                confidence: 0.9,
                constituent_ids: vec![obs_id],
                timestamp: ts,
            }),
            PerceptionEvent::FusionWindowExpired(FusionWindowExpired {
                correlation_key: corr_key.clone(),
                expected_sources: vec!["fs".into(), "net".into()],
                received: vec!["fs".into()],
                timestamp: ts,
            }),
            PerceptionEvent::ObservationReady(ObservationReady {
                observation_id: id,
                modality: Modality::FileSystem,
                priority: ObservationPriority::Normal,
                timestamp: ts,
            }),
            PerceptionEvent::PipelineStageFailed(PipelineStageFailed {
                stage: "normalizer".into(),
                error: "timeout".into(),
                restarts: 3,
                timestamp: ts,
            }),
            PerceptionEvent::PipelineBackpressure(PipelineBackpressure {
                stage: "normalizer".into(),
                channel_capacity: 10000,
                drop_count: 5,
                timestamp: ts,
            }),
            PerceptionEvent::PipelineReconfigured(PipelineReconfigured {
                sections_changed: vec!["attention".into()],
                timestamp: ts,
            }),
            PerceptionEvent::ObserverStatusChanged(ObserverStatusChanged {
                observer_id: "obs-1".into(),
                old_status: "running".into(),
                new_status: "stopped".into(),
                timestamp: ts,
            }),
        ];

        let expected_types = [
            "perception.observation.received",
            "perception.observation.normalized",
            "perception.observation.rejected",
            "perception.observation.filtered",
            "perception.entities.extracted",
            "perception.entities.resolution_failed",
            "perception.context.enriched",
            "perception.context.session_failed",
            "perception.state.changed",
            "perception.state.conflict",
            "perception.anomaly.detected",
            "perception.anomaly.baseline",
            "perception.fusion.complete",
            "perception.fusion.window_expired",
            "perception.observation.ready",
            "perception.pipeline.stage_failed",
            "perception.pipeline.backpressure",
            "perception.pipeline.reconfigured",
            "perception.observer.status",
        ];

        for (event, expected) in events.iter().zip(expected_types.iter()) {
            assert_eq!(
                event.event_type(),
                *expected,
                "event type mismatch for {:?}",
                event
            );
        }
    }

    #[test]
    fn test_event_metadata() {
        let event = PerceptionEvent::ObservationReceived(ObservationReceived {
            observation_id: ObservationId::new(),
            source: ObservationSource {
                observer_id: "test".into(),
                observer_kind: ObserverKind::FileSystem,
                instance_id: "inst-1".into(),
                hostname: "host".into(),
            },
            modality: Modality::FileSystem,
            size_bytes: 100,
            timestamp: Timestamp::now(),
        });
        let meta = event.metadata();
        assert_eq!(
            meta.get("event_type").unwrap(),
            "perception.observation.received"
        );
        assert!(meta.contains_key("observation_id"));
    }
}
