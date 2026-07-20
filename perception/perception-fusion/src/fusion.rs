use async_trait::async_trait;
use memory_core::Timestamp;
use perception_core::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::sync::RwLock;

// ── Types ──────────────────────────────────────────────────

pub type FusionResult<T> = Result<T, FusionError>;

#[derive(Debug, thiserror::Error)]
pub enum FusionError {
    #[error("rule not found: {0}")]
    RuleNotFound(String),
    #[error("correlation window full for key {0:?}")]
    CorrelationWindowFull(CorrelationKey),
    #[error("fusion conflict: {0}")]
    FusionConflict(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrelationRule {
    pub name: String,
    pub correlation_key_expr: String,
    pub expected_sources: Vec<ObserverKind>,
    pub window_ms: u64,
    pub min_observations: usize,
    pub fusion_fn: FusionFn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FusionFn {
    TakeFirst,
    Merge,
    Average,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FusedObservation {
    pub correlation_key: CorrelationKey,
    pub sources: Vec<ObservationSource>,
    pub fused_payload: ObservationPayload,
    pub confidence: Confidence,
    pub synthesized_at: Timestamp,
    pub constituent_ids: Vec<ObservationId>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FusionStats {
    pub active_windows: u64,
    pub completed_fusions: u64,
    pub expired_windows: u64,
    pub conflicts: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FusionConfig {
    pub enabled: bool,
    pub default_window_ms: u64,
    pub confidence_threshold: f64,
}

impl Default for FusionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            default_window_ms: 100,
            confidence_threshold: 0.6,
        }
    }
}

// ── Internal: CorrelationWindow ────────────────────────────

#[derive(Debug, Clone)]
struct CorrelationWindow {
    key: CorrelationKey,
    rule: CorrelationRule,
    observations: Vec<Observation>,
    received_sources: HashSet<ObserverKind>,
}

impl CorrelationWindow {
    fn new(key: CorrelationKey, rule: CorrelationRule) -> Self {
        Self {
            key,
            rule,
            observations: Vec::new(),
            received_sources: HashSet::new(),
        }
    }

    fn add(&mut self, observation: Observation) {
        self.received_sources
            .insert(observation.source.observer_kind.clone());
        self.observations.push(observation);
    }

    fn is_complete(&self) -> bool {
        self.observations.len() >= self.rule.min_observations
            && self
                .rule
                .expected_sources
                .iter()
                .all(|k| self.received_sources.contains(k))
    }

    fn synthesize(&self, timestamp: Timestamp) -> FusedObservation {
        let sources: Vec<ObservationSource> =
            self.observations.iter().map(|o| o.source.clone()).collect();
        let constituent_ids: Vec<ObservationId> = self.observations.iter().map(|o| o.id).collect();

        let fused_payload = match self.rule.fusion_fn {
            FusionFn::TakeFirst => self
                .observations
                .first()
                .map(|o| o.payload.clone())
                .unwrap_or(ObservationPayload::Text {
                    content: String::new(),
                    encoding: "utf-8".into(),
                }),
            FusionFn::Merge => {
                let mut merged_fields: HashMap<String, serde_json::Value> = HashMap::new();
                for obs in &self.observations {
                    if let ObservationPayload::Structured { ref fields } = obs.payload {
                        for (k, v) in fields {
                            merged_fields.insert(k.clone(), v.clone());
                        }
                    }
                }
                if merged_fields.is_empty() {
                    self.observations
                        .first()
                        .map(|o| o.payload.clone())
                        .unwrap_or(ObservationPayload::Text {
                            content: String::new(),
                            encoding: "utf-8".into(),
                        })
                } else {
                    ObservationPayload::Structured {
                        fields: merged_fields,
                    }
                }
            }
            FusionFn::Average => {
                let sum: f64 = self
                    .observations
                    .iter()
                    .filter_map(|o| {
                        if let ObservationPayload::Metric { value, .. } = o.payload {
                            Some(value)
                        } else {
                            None
                        }
                    })
                    .sum();
                let count = self
                    .observations
                    .iter()
                    .filter(|o| matches!(o.payload, ObservationPayload::Metric { .. }))
                    .count();

                if count > 0 {
                    ObservationPayload::Metric {
                        name: "fused".into(),
                        value: sum / count as f64,
                        unit: "avg".into(),
                    }
                } else {
                    self.observations
                        .first()
                        .map(|o| o.payload.clone())
                        .unwrap_or(ObservationPayload::Text {
                            content: String::new(),
                            encoding: "utf-8".into(),
                        })
                }
            }
        };

        let avg_confidence = if self.observations.is_empty() {
            Confidence::DEFAULT
        } else {
            let sum: f32 = self.observations.iter().map(|o| o.confidence.0).sum();
            Confidence((sum / self.observations.len() as f32).clamp(0.0, 1.0))
        };

        FusedObservation {
            correlation_key: self.key.clone(),
            sources,
            fused_payload,
            confidence: avg_confidence,
            synthesized_at: timestamp,
            constituent_ids,
        }
    }
}

// ── FusionEngine trait ─────────────────────────────────────

#[async_trait]
pub trait FusionEngine: Debug + Send + Sync {
    fn register_rule(&self, rule: CorrelationRule) -> FusionResult<()>;

    async fn feed(
        &self,
        observation: Observation,
    ) -> Result<Option<FusedObservation>, PerceptionError>;

    async fn flush(&self) -> Result<Vec<FusedObservation>, PerceptionError>;

    fn stats(&self) -> FusionStats;
}

// ── DefaultFusionEngine ────────────────────────────────────

#[derive(Debug)]
pub struct DefaultFusionEngine {
    config: FusionConfig,
    rules: RwLock<Vec<CorrelationRule>>,
    windows: RwLock<HashMap<CorrelationKey, CorrelationWindow>>,
    stats: RwLock<FusionStats>,
}

impl DefaultFusionEngine {
    pub fn new(config: FusionConfig) -> Self {
        Self {
            config,
            rules: RwLock::new(Vec::new()),
            windows: RwLock::new(HashMap::new()),
            stats: RwLock::new(FusionStats::default()),
        }
    }

    fn extract_key(
        &self,
        observation: &Observation,
        rule: &CorrelationRule,
    ) -> Option<CorrelationKey> {
        if let Some(cid) = &observation.correlation_id {
            return Some(CorrelationKey(format!("cid:{}", cid)));
        }
        let fallback = format!(
            "{}:{:?}",
            observation.source.instance_id, observation.modality
        );
        let key = observation
            .metadata
            .get(&rule.correlation_key_expr)
            .cloned()
            .unwrap_or(fallback);
        Some(CorrelationKey(key))
    }
}

#[async_trait]
impl FusionEngine for DefaultFusionEngine {
    fn register_rule(&self, rule: CorrelationRule) -> FusionResult<()> {
        if let Ok(mut rules) = self.rules.write() {
            rules.push(rule);
        }
        Ok(())
    }

    async fn feed(
        &self,
        observation: Observation,
    ) -> Result<Option<FusedObservation>, PerceptionError> {
        if !self.config.enabled {
            return Ok(None);
        }

        let matching_rules: Vec<CorrelationRule> = self
            .rules
            .read()
            .ok()
            .map(|rules| {
                rules
                    .iter()
                    .filter(|r| {
                        r.expected_sources
                            .contains(&observation.source.observer_kind)
                    })
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();

        if matching_rules.is_empty() {
            return Ok(None);
        }

        for rule in &matching_rules {
            let Some(key) = self.extract_key(&observation, rule) else {
                continue;
            };

            if let Ok(mut windows) = self.windows.write() {
                let window = windows
                    .entry(key.clone())
                    .or_insert_with(|| CorrelationWindow::new(key.clone(), rule.clone()));

                window.add(observation.clone());

                if window.is_complete() {
                    let ts = Timestamp::now();
                    let fused = window.synthesize(ts);
                    windows.remove(&key);

                    if let Ok(mut s) = self.stats.write() {
                        s.completed_fusions += 1;
                        s.active_windows = windows.len() as u64;
                    }

                    return Ok(Some(fused));
                }

                if let Ok(mut s) = self.stats.write() {
                    s.active_windows = windows.len() as u64;
                }
            }
        }

        Ok(None)
    }

    async fn flush(&self) -> Result<Vec<FusedObservation>, PerceptionError> {
        let windows: Vec<(CorrelationKey, CorrelationWindow)> = self
            .windows
            .write()
            .ok()
            .map(|mut w| w.drain().collect())
            .unwrap_or_default();

        let mut fused = Vec::new();
        let total = windows.len();
        for (_key, window) in windows {
            if !window.observations.is_empty() {
                fused.push(window.synthesize(Timestamp::now()));
            }
        }

        if let Ok(mut s) = self.stats.write() {
            s.completed_fusions += fused.len() as u64;
            s.expired_windows += total as u64;
            s.active_windows = 0;
        }

        Ok(fused)
    }

    fn stats(&self) -> FusionStats {
        self.stats.read().map(|s| s.clone()).unwrap_or_default()
    }
}

// ── Tests ──────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use perception_core::ObservationPriority;

    fn make_observation(
        source: ObservationSource,
        modality: Modality,
        payload: ObservationPayload,
        correlation_id: Option<CorrelationId>,
    ) -> Observation {
        let mut obs = Observation::new(source, modality, ObservationPriority::Normal, payload);
        obs.correlation_id = correlation_id;
        obs
    }

    #[tokio::test]
    async fn test_register_rule() {
        let engine = DefaultFusionEngine::new(FusionConfig::default());
        let rule = CorrelationRule {
            name: "test-rule".into(),
            correlation_key_expr: "correlation_id".into(),
            expected_sources: vec![ObserverKind::FileSystem],
            window_ms: 1000,
            min_observations: 2,
            fusion_fn: FusionFn::TakeFirst,
        };
        engine.register_rule(rule).unwrap();
        let stats = engine.stats();
        assert_eq!(stats.active_windows, 0);
    }

    #[tokio::test]
    async fn test_feed_unmatched_observation_returns_none() {
        let engine = DefaultFusionEngine::new(FusionConfig::default());
        let rule = CorrelationRule {
            name: "fs-rule".into(),
            correlation_key_expr: "correlation_id".into(),
            expected_sources: vec![ObserverKind::FileSystem],
            window_ms: 10000,
            min_observations: 1,
            fusion_fn: FusionFn::TakeFirst,
        };
        engine.register_rule(rule).unwrap();

        let source = ObservationSource {
            observer_id: "proc".into(),
            observer_kind: ObserverKind::Process,
            instance_id: "p1".into(),
            hostname: "localhost".into(),
        };
        let obs = make_observation(
            source,
            Modality::Process,
            ObservationPayload::Text {
                content: "test".into(),
                encoding: "utf-8".into(),
            },
            None,
        );

        let result = engine.feed(obs).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_complete_fusion_with_two_observations() {
        let engine = DefaultFusionEngine::new(FusionConfig::default());
        let rule = CorrelationRule {
            name: "test-rule".into(),
            correlation_key_expr: "cid".into(),
            expected_sources: vec![ObserverKind::FileSystem, ObserverKind::Process],
            window_ms: 10000,
            min_observations: 2,
            fusion_fn: FusionFn::Merge,
        };
        engine.register_rule(rule).unwrap();

        let cid = CorrelationId::new();
        let fs_source = ObservationSource {
            observer_id: "fs".into(),
            observer_kind: ObserverKind::FileSystem,
            instance_id: "fs-1".into(),
            hostname: "localhost".into(),
        };
        let proc_source = ObservationSource {
            observer_id: "proc".into(),
            observer_kind: ObserverKind::Process,
            instance_id: "p1".into(),
            hostname: "localhost".into(),
        };

        let obs1 = make_observation(
            fs_source,
            Modality::FileSystem,
            ObservationPayload::Structured {
                fields: [("path".into(), serde_json::Value::String("/tmp/test".into()))].into(),
            },
            Some(cid),
        );

        let obs2 = make_observation(
            proc_source,
            Modality::Process,
            ObservationPayload::Structured {
                fields: [("pid".into(), serde_json::Value::Number(1234.into()))].into(),
            },
            Some(cid),
        );

        let result1 = engine.feed(obs1).await.unwrap();
        assert!(result1.is_none()); // not yet complete

        let result2 = engine.feed(obs2).await.unwrap();
        assert!(result2.is_some());

        let fused = result2.unwrap();
        assert_eq!(fused.constituent_ids.len(), 2);
        assert_eq!(fused.sources.len(), 2);
    }

    #[tokio::test]
    async fn test_fusion_disabled_returns_none() {
        let config = FusionConfig {
            enabled: false,
            ..FusionConfig::default()
        };
        let engine = DefaultFusionEngine::new(config);
        let rule = CorrelationRule {
            name: "test-rule".into(),
            correlation_key_expr: "key".into(),
            expected_sources: vec![ObserverKind::FileSystem],
            window_ms: 1000,
            min_observations: 1,
            fusion_fn: FusionFn::TakeFirst,
        };
        engine.register_rule(rule).unwrap();

        let source = ObservationSource {
            observer_id: "fs".into(),
            observer_kind: ObserverKind::FileSystem,
            instance_id: "fs-1".into(),
            hostname: "localhost".into(),
        };
        let obs = make_observation(
            source,
            Modality::FileSystem,
            ObservationPayload::Text {
                content: "test".into(),
                encoding: "utf-8".into(),
            },
            None,
        );

        let result = engine.feed(obs).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_flush_returns_pending_observations() {
        let engine = DefaultFusionEngine::new(FusionConfig::default());
        let rule = CorrelationRule {
            name: "test-rule".into(),
            correlation_key_expr: "key".into(),
            expected_sources: vec![ObserverKind::FileSystem],
            window_ms: 100000,
            min_observations: 2,
            fusion_fn: FusionFn::TakeFirst,
        };
        engine.register_rule(rule).unwrap();

        let source = ObservationSource {
            observer_id: "fs".into(),
            observer_kind: ObserverKind::FileSystem,
            instance_id: "fs-1".into(),
            hostname: "localhost".into(),
        };
        let obs = make_observation(
            source,
            Modality::FileSystem,
            ObservationPayload::Text {
                content: "test".into(),
                encoding: "utf-8".into(),
            },
            Some(CorrelationId::new()),
        );

        engine.feed(obs).await.unwrap();

        let flushed = engine.flush().await.unwrap();
        assert_eq!(flushed.len(), 1);
    }

    #[tokio::test]
    async fn test_average_fusion_fn() {
        let engine = DefaultFusionEngine::new(FusionConfig::default());
        let rule = CorrelationRule {
            name: "avg-rule".into(),
            correlation_key_expr: "key".into(),
            expected_sources: vec![ObserverKind::FileSystem],
            window_ms: 10000,
            min_observations: 3,
            fusion_fn: FusionFn::Average,
        };
        engine.register_rule(rule).unwrap();

        let base_source = ObservationSource {
            observer_id: "fs".into(),
            observer_kind: ObserverKind::FileSystem,
            instance_id: "fs-1".into(),
            hostname: "localhost".into(),
        };

        let cid = CorrelationId::new();
        for value in [10.0, 20.0, 30.0] {
            let mut obs = Observation::new(
                base_source.clone(),
                Modality::FileSystem,
                ObservationPriority::Normal,
                ObservationPayload::Metric {
                    name: "test".into(),
                    value,
                    unit: "count".into(),
                },
            );
            obs.correlation_id = Some(cid);
            engine.feed(obs).await.unwrap();
        }

        let stats = engine.stats();
        assert_eq!(stats.completed_fusions, 1);
    }

    #[tokio::test]
    async fn test_stats_tracking() {
        let engine = DefaultFusionEngine::new(FusionConfig::default());

        let stats = engine.stats();
        assert_eq!(stats.active_windows, 0);
        assert_eq!(stats.completed_fusions, 0);
        assert_eq!(stats.expired_windows, 0);
        assert_eq!(stats.conflicts, 0);
    }
}

// ── Benchmarks ─────────────────────────────────────────────

#[cfg(debug_assertions)]
pub mod benchmarks {
    use super::*;
    use perception_core::ObservationPriority;

    pub fn bench_fusion_complete_pair() {
        let engine = DefaultFusionEngine::new(FusionConfig::default());
        engine
            .register_rule(CorrelationRule {
                name: "bench-rule".into(),
                correlation_key_expr: "key".into(),
                expected_sources: vec![ObserverKind::FileSystem],
                window_ms: 10000,
                min_observations: 2,
                fusion_fn: FusionFn::Merge,
            })
            .unwrap();

        let source = ObservationSource {
            observer_id: "bench".into(),
            observer_kind: ObserverKind::FileSystem,
            instance_id: "b1".into(),
            hostname: "bench".into(),
        };

        let start = std::time::Instant::now();
        let iterations = 10_000;

        for _ in 0..iterations {
            let cid = CorrelationId::new();
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let mut obs1 = Observation::new(
                    source.clone(),
                    Modality::FileSystem,
                    ObservationPriority::Normal,
                    ObservationPayload::Text {
                        content: "a".into(),
                        encoding: "utf-8".into(),
                    },
                );
                obs1.correlation_id = Some(cid);
                engine.feed(obs1).await.unwrap();

                let mut obs2 = Observation::new(
                    source.clone(),
                    Modality::FileSystem,
                    ObservationPriority::Normal,
                    ObservationPayload::Text {
                        content: "b".into(),
                        encoding: "utf-8".into(),
                    },
                );
                obs2.correlation_id = Some(cid);
                engine.feed(obs2).await.unwrap();
            });
        }

        let elapsed = start.elapsed();
        let ops = iterations as f64 / elapsed.as_secs_f64();
        println!("Fusion throughput: {:.0} ops/s", ops);
    }
}
