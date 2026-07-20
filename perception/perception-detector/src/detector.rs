use async_trait::async_trait;
use memory_core::Timestamp;
use perception_core::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::RwLock;

pub type DetectorResult<T> = Result<T, DetectorError>;

#[derive(Debug, thiserror::Error)]
pub enum DetectorError {
    #[error("state machine not found: {0}")]
    StateMachineNotFound(String),
    #[error("state machine conflict: {0}")]
    StateMachineConflict(String),
    #[error("invalid state transition: {from} -> {to}")]
    InvalidTransition { from: String, to: String },
    #[error("metric not found: {0}")]
    MetricNotFound(String),
    #[error("clock error: {0}")]
    ClockError(String),
}

// ── State Machine Types ───────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateMachine {
    pub name: String,
    pub initial_state: String,
    pub states: Vec<StateDefinition>,
    pub transitions: Vec<TransitionRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateDefinition {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransitionRule {
    pub from: String,
    pub to: String,
    pub condition: String,
    pub on_transition: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateTransition {
    pub scope: ScopeId,
    pub from: String,
    pub to: String,
    pub triggered_by: ObservationId,
    pub timestamp: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectorConfig {
    pub default_state: String,
    pub max_scopes: usize,
    pub track_metrics: bool,
}

impl Default for DetectorConfig {
    fn default() -> Self {
        Self {
            default_state: "initial".into(),
            max_scopes: 10000,
            track_metrics: true,
        }
    }
}

// ── StateDetector trait ───────────────────────────────────

#[async_trait]
pub trait StateDetector: Debug + Send + Sync {
    fn register_machine(
        &self,
        scope: ScopeId,
        machine: StateMachine,
    ) -> DetectorResult<()>;

    async fn detect(
        &self,
        observation: &Observation,
    ) -> Result<Option<StateTransition>, perception_core::PerceptionError>;

    fn current_state(&self, scope: &ScopeId) -> DetectorResult<Option<String>>;

    fn reset(&self, scope: &ScopeId) -> DetectorResult<()>;
}

// ── DefaultStateDetector ──────────────────────────────────

#[derive(Debug)]
pub struct DefaultStateDetector {
    #[allow(dead_code)]
    config: DetectorConfig,
    machines: RwLock<HashMap<ScopeId, StateMachine>>,
    states: RwLock<HashMap<ScopeId, String>>,
}

impl DefaultStateDetector {
    pub fn new(config: DetectorConfig) -> Self {
        Self {
            config,
            machines: RwLock::new(HashMap::new()),
            states: RwLock::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl StateDetector for DefaultStateDetector {
    fn register_machine(
        &self,
        scope: ScopeId,
        machine: StateMachine,
    ) -> DetectorResult<()> {
        let initial = machine.initial_state.clone();
        if let Ok(mut machines) = self.machines.write() {
            machines.insert(scope.clone(), machine);
        }
        if let Ok(mut states) = self.states.write() {
            states.insert(scope, initial);
        }
        Ok(())
    }

    async fn detect(
        &self,
        observation: &Observation,
    ) -> Result<Option<StateTransition>, perception_core::PerceptionError> {
        // Extract scope from observation metadata
        let scope_key = observation
            .metadata
            .get("scope")
            .cloned()
            .unwrap_or_else(|| "default".to_string());
        let scope = ScopeId(scope_key);

        // Get current state and machine
        let current = self
            .states
            .read()
            .map_err(|_| {
                perception_core::PerceptionError::ConfigurationError(
                    "state lock poisoned".into(),
                )
            })?
            .get(&scope)
            .cloned();

        let machine = self
            .machines
            .read()
            .map_err(|_| {
                perception_core::PerceptionError::ConfigurationError(
                    "machine lock poisoned".into(),
                )
            })?
            .get(&scope)
            .cloned();

        let Some(current_state) = current else {
            return Ok(None);
        };
        let Some(machine) = machine else {
            return Ok(None);
        };

        // Evaluate transition rules
        for rule in &machine.transitions {
            if rule.from == current_state {
                let modality_match = format!("{:?}", observation.modality).contains(&rule.condition)
                    || payload_to_string(&observation.payload).contains(&rule.condition);

                if modality_match {
                    // Transition found
                    let transition = StateTransition {
                        scope: scope.clone(),
                        from: current_state.clone(),
                        to: rule.to.clone(),
                        triggered_by: observation.id,
                        timestamp: Timestamp::now(),
                    };

                    // Update state
                    if let Ok(mut states) = self.states.write() {
                        states.insert(scope.clone(), rule.to.clone());
                    }

                    return Ok(Some(transition));
                }
            }
        }

        Ok(None)
    }

    fn current_state(&self, scope: &ScopeId) -> DetectorResult<Option<String>> {
        Ok(self
            .states
            .read()
            .map_err(|_| DetectorError::StateMachineNotFound(scope.to_string()))?
            .get(scope)
            .cloned())
    }

    fn reset(&self, scope: &ScopeId) -> DetectorResult<()> {
        let machine = self
            .machines
            .read()
            .map_err(|_| DetectorError::StateMachineNotFound(scope.to_string()))?
            .get(scope)
            .cloned();
        if let Some(machine) = machine {
            if let Ok(mut states) = self.states.write() {
                states.insert(scope.clone(), machine.initial_state);
            }
            Ok(())
        } else {
            Err(DetectorError::StateMachineNotFound(scope.to_string()))
        }
    }
}

// ── ChangeDetector types ──────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeDetectorConfig {
    pub window_seconds: u64,
    pub absolute_threshold: Option<f64>,
    pub relative_threshold: Option<f64>,
    pub rate_threshold: Option<f64>,
}

impl Default for ChangeDetectorConfig {
    fn default() -> Self {
        Self {
            window_seconds: 60,
            absolute_threshold: None,
            relative_threshold: Some(0.1),
            rate_threshold: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeEvent {
    pub key: String,
    pub old_value: f64,
    pub new_value: f64,
    pub rate: f64,
    pub detected_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricStats {
    pub min: f64,
    pub max: f64,
    pub mean: f64,
    pub variance: f64,
    pub count: u64,
}

// ── ChangeDetector trait ──────────────────────────────────

#[async_trait]
pub trait ChangeDetector: Debug + Send + Sync {
    fn register_metric(
        &self,
        key: String,
        config: ChangeDetectorConfig,
    ) -> DetectorResult<()>;

    async fn feed(
        &self,
        key: &str,
        value: f64,
        timestamp: Timestamp,
    ) -> Result<Option<ChangeEvent>, perception_core::PerceptionError>;

    fn stats(&self, key: &str) -> DetectorResult<Option<MetricStats>>;
}

// ── DefaultChangeDetector ─────────────────────────────────

#[derive(Debug)]
struct MetricWindow {
    config: ChangeDetectorConfig,
    values: Vec<(Timestamp, f64)>,
    min: f64,
    max: f64,
    sum: f64,
    sum_sq: f64,
}

#[derive(Debug)]
pub struct DefaultChangeDetector {
    metrics: RwLock<HashMap<String, MetricWindow>>,
}

impl DefaultChangeDetector {
    pub fn new() -> Self {
        Self {
            metrics: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for DefaultChangeDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ChangeDetector for DefaultChangeDetector {
    fn register_metric(
        &self,
        key: String,
        config: ChangeDetectorConfig,
    ) -> DetectorResult<()> {
        let mut metrics = self
            .metrics
            .write()
            .map_err(|_| DetectorError::ClockError("lock poisoned".into()))?;
        metrics.insert(
            key,
            MetricWindow {
                config,
                values: Vec::new(),
                min: f64::MAX,
                max: f64::MIN,
                sum: 0.0,
                sum_sq: 0.0,
            },
        );
        Ok(())
    }

    async fn feed(
        &self,
        key: &str,
        value: f64,
        timestamp: Timestamp,
    ) -> Result<Option<ChangeEvent>, perception_core::PerceptionError> {
        let mut metrics = self
            .metrics
            .write()
            .map_err(|_| {
                perception_core::PerceptionError::ConfigurationError(
                    "metrics lock poisoned".into(),
                )
            })?;
        let Some(window) = metrics.get_mut(key) else {
            return Ok(None);
        };

        // Prune old values outside the window
        let cutoff = Timestamp::from_nanos(
            timestamp.as_nanos() - (window.config.window_seconds as i64 * 1_000_000_000),
        );
        window.values.retain(|(ts, _)| *ts >= cutoff);

        let old_count = window.values.len();
        let old_mean = if old_count == 0 {
            0.0
        } else {
            window.sum / old_count as f64
        };

        // Add new value
        window.values.push((timestamp, value));
        window.min = window.min.min(value);
        window.max = window.max.max(value);
        window.sum += value;
        window.sum_sq += value * value;

        let mut change_event = None;

        if old_count > 0 {
            if let Some(abs_threshold) = window.config.absolute_threshold {
                if (value - old_mean).abs() > abs_threshold {
                    change_event = Some(ChangeEvent {
                        key: key.to_string(),
                        old_value: old_mean,
                        new_value: value,
                        rate: if old_mean != 0.0 {
                            (value - old_mean) / old_mean
                        } else {
                            0.0
                        },
                        detected_at: timestamp,
                    });
                }
            }

            if change_event.is_none() {
                if let Some(rel_threshold) = window.config.relative_threshold {
                    if old_mean != 0.0 && ((value - old_mean) / old_mean).abs() > rel_threshold {
                        change_event = Some(ChangeEvent {
                            key: key.to_string(),
                            old_value: old_mean,
                            new_value: value,
                            rate: (value - old_mean) / old_mean,
                            detected_at: timestamp,
                        });
                    }
                }
            }
        }

        Ok(change_event)
    }

    fn stats(&self, key: &str) -> DetectorResult<Option<MetricStats>> {
        let metrics = self
            .metrics
            .read()
            .map_err(|_| DetectorError::MetricNotFound(key.to_string()))?;
        Ok(metrics.get(key).map(|w| {
            let count = w.values.len() as u64;
            let mean = if count > 0 { w.sum / count as f64 } else { 0.0 };
            let variance = if count > 1 {
                (w.sum_sq - (w.sum * w.sum) / count as f64) / (count - 1) as f64
            } else {
                0.0
            };
            MetricStats {
                min: w.min,
                max: w.max,
                mean,
                variance,
                count,
            }
        }))
    }
}

// ── Helpers ───────────────────────────────────────────────

fn payload_to_string(payload: &ObservationPayload) -> String {
    match payload {
        ObservationPayload::Text { content, .. } => content.clone(),
        ObservationPayload::Structured { fields } => {
            serde_json::to_string(fields).unwrap_or_default()
        }
        ObservationPayload::Event { event_type, .. } => event_type.clone(),
        ObservationPayload::State { key, .. } => key.clone(),
        ObservationPayload::Metric { name, .. } => name.clone(),
        ObservationPayload::Binary { mime_type, .. } => mime_type.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_state_machine_lifecycle() {
        let detector = DefaultStateDetector::new(DetectorConfig::default());
        let scope = ScopeId("session-1".into());

        let machine = StateMachine {
            name: "session".into(),
            initial_state: "idle".into(),
            states: vec![
                StateDefinition {
                    name: "idle".into(),
                    description: "no activity".into(),
                },
                StateDefinition {
                    name: "active".into(),
                    description: "activity detected".into(),
                },
            ],
            transitions: vec![TransitionRule {
                from: "idle".into(),
                to: "active".into(),
                condition: "Terminal".into(),
                on_transition: Some("session_activated".into()),
            }],
        };

        detector.register_machine(scope.clone(), machine).unwrap();
        assert_eq!(
            detector.current_state(&scope).unwrap(),
            Some("idle".into())
        );

        let source = ObservationSource {
            observer_id: "test".into(),
            observer_kind: ObserverKind::Terminal,
            instance_id: "inst-1".into(),
            hostname: "localhost".into(),
        };
        let mut obs = Observation::new(
            source,
            Modality::Terminal,
            ObservationPriority::Normal,
            ObservationPayload::Text {
                content: "user input".into(),
                encoding: "utf-8".into(),
            },
        );
        obs.metadata.insert("scope".into(), "session-1".into());

        let transition = detector.detect(&obs).await.unwrap();
        assert!(transition.is_some());
        let t = transition.unwrap();
        assert_eq!(t.from, "idle");
        assert_eq!(t.to, "active");

        assert_eq!(
            detector.current_state(&scope).unwrap(),
            Some("active".into())
        );

        detector.reset(&scope).unwrap();
        assert_eq!(
            detector.current_state(&scope).unwrap(),
            Some("idle".into())
        );
    }

    #[tokio::test]
    async fn test_change_detection() {
        let detector = DefaultChangeDetector::new();
        let ts = Timestamp::now();

        detector
            .register_metric(
                "cpu".into(),
                ChangeDetectorConfig {
                    window_seconds: 60,
                    absolute_threshold: Some(10.0),
                    relative_threshold: None,
                    rate_threshold: None,
                },
            )
            .unwrap();

        // No change for small delta
        let result = detector.feed("cpu", 50.0, ts).await.unwrap();
        assert!(result.is_none());

        let result = detector
            .feed("cpu", 51.0, Timestamp::from_nanos(ts.as_nanos() + 1_000_000_000))
            .await
            .unwrap();
        assert!(result.is_none());

        // Large delta should trigger change
        let result = detector
            .feed("cpu", 70.0, Timestamp::from_nanos(ts.as_nanos() + 2_000_000_000))
            .await
            .unwrap();
        assert!(result.is_some());
        let change = result.unwrap();
        assert_eq!(change.key, "cpu");
    }

    #[tokio::test]
    async fn test_metric_stats() {
        let detector = DefaultChangeDetector::new();
        let ts = Timestamp::now();

        detector
            .register_metric(
                "memory".into(),
                ChangeDetectorConfig::default(),
            )
            .unwrap();

        detector.feed("memory", 100.0, ts).await.unwrap();
        detector
            .feed("memory", 200.0, Timestamp::from_nanos(ts.as_nanos() + 1_000_000_000))
            .await
            .unwrap();

        let stats = detector.stats("memory").unwrap().unwrap();
        assert_eq!(stats.count, 2);
        assert!((stats.mean - 150.0).abs() < 0.01);
    }
}
