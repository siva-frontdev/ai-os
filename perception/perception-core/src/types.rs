use memory_core::Timestamp;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

// ── ObservationId ─────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ObservationId(uuid::Uuid);

impl ObservationId {
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

impl Default for ObservationId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ObservationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for ObservationId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        uuid::Uuid::from_str(s).map(Self)
    }
}

// ── CorrelationId ─────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CorrelationId(uuid::Uuid);

impl CorrelationId {
    pub fn new() -> Self {
        Self(uuid::Uuid::now_v7())
    }

    pub const fn from_uuid(uuid: uuid::Uuid) -> Self {
        Self(uuid)
    }

    pub const fn as_uuid(&self) -> &uuid::Uuid {
        &self.0
    }
}

impl Default for CorrelationId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for CorrelationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

// ── CorrelationKey ────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CorrelationKey(pub String);

impl fmt::Display for CorrelationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

// ── ScopeId ───────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ScopeId(pub String);

impl fmt::Display for ScopeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

// ── ObserverKind ──────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ObserverKind {
    FileSystem,
    Process,
    Terminal,
    Network,
    SystemEvent,
    Hardware,
    TimeSeries,
    Custom(String),
}

// ── Modality ──────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Modality {
    Terminal,
    FileSystem,
    Network,
    Process,
    UserInput,
    SystemEvent,
    Hardware,
    TimeSeries,
    Custom(String),
}

// ── ObservationPriority ───────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ObservationPriority {
    Critical = 0,
    High = 1,
    Normal = 2,
    Low = 3,
    Background = 4,
}

impl ObservationPriority {
    pub fn rank(&self) -> u8 {
        match self {
            Self::Critical => 0,
            Self::High => 1,
            Self::Normal => 2,
            Self::Low => 3,
            Self::Background => 4,
        }
    }
}

// ── Confidence ────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Confidence(pub f32);

impl Confidence {
    pub const MIN: Self = Self(0.0);
    pub const MAX: Self = Self(1.0);
    pub const DEFAULT: Self = Self(0.5);

    pub fn new(value: f32) -> Self {
        Self(value.clamp(0.0, 1.0))
    }

    pub const fn raw(&self) -> f32 {
        self.0
    }

    pub const fn from_raw(value: f32) -> Self {
        Self(value)
    }
}

impl Default for Confidence {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl fmt::Display for Confidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.4}", self.0)
    }
}

// ── ObservationSource ─────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservationSource {
    pub observer_id: String,
    pub observer_kind: ObserverKind,
    pub instance_id: String,
    pub hostname: String,
}

// ── ObservationPayload ────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ObservationPayload {
    Text {
        content: String,
        encoding: String,
    },
    Binary {
        content: Vec<u8>,
        mime_type: String,
    },
    Structured {
        fields: HashMap<String, serde_json::Value>,
    },
    Event {
        event_type: String,
        data: serde_json::Value,
    },
    State {
        key: String,
        old_value: Option<String>,
        new_value: String,
    },
    Metric {
        name: String,
        value: f64,
        unit: String,
    },
}

// ── TransformationStep ────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransformationStep {
    pub stage: String,
    pub timestamp: Timestamp,
    pub description: String,
}

// ── Provenance ────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provenance {
    pub raw_bytes: Option<Vec<u8>>,
    pub original_format: String,
    pub transformation_log: Vec<TransformationStep>,
}

// ── QualityScore ──────────────────────────────────────────

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct QualityScore {
    pub completeness: f32,
    pub timeliness: f32,
    pub signal_to_noise: f32,
    pub overall: f32,
}

impl QualityScore {
    pub fn new(completeness: f32, timeliness: f32, signal_to_noise: f32) -> Self {
        let overall = (completeness + timeliness + signal_to_noise) / 3.0;
        Self {
            completeness,
            timeliness,
            signal_to_noise,
            overall,
        }
    }
}

impl Default for QualityScore {
    fn default() -> Self {
        Self {
            completeness: 1.0,
            timeliness: 1.0,
            signal_to_noise: 1.0,
            overall: 1.0,
        }
    }
}

// ── Observation ───────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    pub id: ObservationId,
    pub timestamp: Timestamp,
    pub source: ObservationSource,
    pub modality: Modality,
    pub confidence: Confidence,
    pub priority: ObservationPriority,
    pub correlation_id: Option<CorrelationId>,
    pub parent_id: Option<ObservationId>,
    pub derived_ids: Vec<ObservationId>,
    pub payload: ObservationPayload,
    pub metadata: HashMap<String, String>,
    pub provenance: Provenance,
    pub quality: QualityScore,
}

impl Observation {
    pub fn new(
        source: ObservationSource,
        modality: Modality,
        priority: ObservationPriority,
        payload: ObservationPayload,
    ) -> Self {
        Self {
            id: ObservationId::new(),
            timestamp: Timestamp::now(),
            source,
            modality,
            confidence: Confidence::DEFAULT,
            priority,
            correlation_id: None,
            parent_id: None,
            derived_ids: Vec::new(),
            payload,
            metadata: HashMap::new(),
            provenance: Provenance {
                raw_bytes: None,
                original_format: String::new(),
                transformation_log: Vec::new(),
            },
            quality: QualityScore::default(),
        }
    }
}

// ── ObservationFilter ─────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservationFilter {
    pub modality: Option<Modality>,
    pub min_priority: Option<ObservationPriority>,
    pub since: Option<Timestamp>,
    pub until: Option<Timestamp>,
    pub source_id: Option<String>,
    pub correlation_id: Option<CorrelationId>,
    pub limit: Option<usize>,
}

// ── MemoryId re-export (alias for compatibility) ──────────

pub use memory_core::MemoryId;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_observation_id_unique() {
        let a = ObservationId::new();
        let b = ObservationId::new();
        assert_ne!(a, b);
    }

    #[test]
    fn test_observation_id_display_and_parse() {
        let id = ObservationId::new();
        let s = id.to_string();
        let parsed: ObservationId = s.parse().unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn test_confidence_clamping() {
        assert_eq!(Confidence::new(1.5).raw(), 1.0);
        assert_eq!(Confidence::new(-0.5).raw(), 0.0);
        assert_eq!(Confidence::new(0.7).raw(), 0.7);
    }

    #[test]
    fn test_priority_rank() {
        assert_eq!(ObservationPriority::Critical.rank(), 0);
        assert_eq!(ObservationPriority::Background.rank(), 4);
    }

    #[test]
    fn test_observation_construction() {
        let source = ObservationSource {
            observer_id: "test".into(),
            observer_kind: ObserverKind::Terminal,
            instance_id: "inst-1".into(),
            hostname: "localhost".into(),
        };
        let obs = Observation::new(
            source,
            Modality::Terminal,
            ObservationPriority::Normal,
            ObservationPayload::Text {
                content: "hello".into(),
                encoding: "utf-8".into(),
            },
        );
        assert_eq!(obs.modality, Modality::Terminal);
        assert_eq!(obs.priority, ObservationPriority::Normal);
    }

    #[test]
    fn test_quality_score_computation() {
        let q = QualityScore::new(0.8, 0.6, 0.4);
        assert!((q.overall - 0.6).abs() < 0.001);
    }

    #[test]
    fn test_correlation_id_default() {
        let id = CorrelationId::default();
        let id2 = CorrelationId::default();
        assert_ne!(id, id2);
    }
}
