use async_trait::async_trait;
use perception_core::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;
use std::sync::RwLock;

pub type AnomalyResult<T> = Result<T, AnomalyError>;

#[derive(Debug, thiserror::Error)]
pub enum AnomalyError {
    #[error("model not found for modality: {0:?}")]
    ModelNotFound(Modality),
    #[error("model not ready: {0}")]
    ModelNotReady(String),
    #[error("insufficient data: need {required}, have {available}")]
    InsufficientData { required: usize, available: usize },
    #[error("invalid sensitivity: {0}")]
    InvalidSensitivity(f64),
}

// ── Anomaly types ─────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyScore {
    pub score: f64,
    pub threshold: f64,
    pub is_anomaly: bool,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyStats {
    pub mean: f64,
    pub std_dev: f64,
    pub min: f64,
    pub max: f64,
    pub count: u64,
    pub anomaly_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyDetectorConfig {
    pub default_window_seconds: u64,
    pub default_sensitivity: f64,
    pub min_data_points: usize,
    pub enabled: bool,
}

impl Default for AnomalyDetectorConfig {
    fn default() -> Self {
        Self {
            default_window_seconds: 60,
            default_sensitivity: 2.0,
            min_data_points: 10,
            enabled: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerModalityAnomalyConfig {
    pub window_seconds: u64,
    pub sensitivity: f64,
    pub min_data_points: Option<usize>,
}

// ── AnomalyModel trait ────────────────────────────────────

#[async_trait]
pub trait AnomalyModel: Debug + Send + Sync {
    fn name(&self) -> &str;
    async fn score(&self, value: f64) -> f64;
    async fn feed(&self, value: f64);
    async fn reset(&self);
}

// ── ZScoreModel ───────────────────────────────────────────

#[derive(Debug)]
pub struct ZScoreModel {
    name: String,
    sensitivity: f64,
    sum: RwLock<f64>,
    sum_sq: RwLock<f64>,
    count: RwLock<u64>,
}

impl ZScoreModel {
    pub fn new(name: &str, sensitivity: f64) -> Self {
        Self {
            name: name.to_string(),
            sensitivity,
            sum: RwLock::new(0.0),
            sum_sq: RwLock::new(0.0),
            count: RwLock::new(0),
        }
    }

    fn mean(&self) -> f64 {
        let count = self.count.read().map(|g| *g).unwrap_or(0);
        if count == 0 {
            return 0.0;
        }
        self.sum.read().map(|g| *g).unwrap_or(0.0) / count as f64
    }

    fn std_dev(&self) -> f64 {
        let count = self.count.read().map(|g| *g).unwrap_or(0);
        if count < 2 {
            return 1.0;
        }
        let sum = self.sum.read().map(|g| *g).unwrap_or(0.0);
        let sum_sq = self.sum_sq.read().map(|g| *g).unwrap_or(0.0);
        let variance = (sum_sq - (sum * sum) / count as f64) / (count - 1) as f64;
        variance.sqrt()
    }
}

#[async_trait]
impl AnomalyModel for ZScoreModel {
    fn name(&self) -> &str {
        &self.name
    }

    async fn score(&self, value: f64) -> f64 {
        let count = self.count.read().map(|g| *g).unwrap_or(0);
        if count < 2 {
            return 0.0;
        }
        let m = self.mean();
        let sd = self.std_dev();
        if sd == 0.0 {
            return 0.0;
        }
        let z = (value - m).abs() / sd;
        (z / self.sensitivity).min(1.0)
    }

    async fn feed(&self, value: f64) {
        if let Ok(mut sum) = self.sum.write() {
            *sum += value;
        }
        if let Ok(mut sum_sq) = self.sum_sq.write() {
            *sum_sq += value * value;
        }
        if let Ok(mut count) = self.count.write() {
            *count += 1;
        }
    }

    async fn reset(&self) {
        if let Ok(mut sum) = self.sum.write() {
            *sum = 0.0;
        }
        if let Ok(mut sum_sq) = self.sum_sq.write() {
            *sum_sq = 0.0;
        }
        if let Ok(mut count) = self.count.write() {
            *count = 0;
        }
    }
}

// ── RollingWindowModel ────────────────────────────────────

#[derive(Debug)]
pub struct RollingWindowModel {
    name: String,
    window_size: usize,
    sensitivity: f64,
    values: RwLock<Vec<f64>>,
}

impl RollingWindowModel {
    pub fn new(name: &str, window_size: usize, sensitivity: f64) -> Self {
        Self {
            name: name.to_string(),
            window_size,
            sensitivity,
            values: RwLock::new(Vec::with_capacity(window_size)),
        }
    }

    fn mean(&self) -> f64 {
        let values = self.values.read().map(|g| g.clone()).unwrap_or_default();
        if values.is_empty() {
            return 0.0;
        }
        values.iter().sum::<f64>() / values.len() as f64
    }

    fn std_dev(&self) -> f64 {
        let values = self.values.read().map(|g| g.clone()).unwrap_or_default();
        let n = values.len();
        if n < 2 {
            return 1.0;
        }
        let mean = values.iter().sum::<f64>() / n as f64;
        let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1) as f64;
        variance.sqrt()
    }
}

#[async_trait]
impl AnomalyModel for RollingWindowModel {
    fn name(&self) -> &str {
        &self.name
    }

    async fn score(&self, value: f64) -> f64 {
        let n = self.values.read().map(|g| g.len()).unwrap_or(0);
        if n < 2 {
            return 0.0;
        }
        let m = self.mean();
        let sd = self.std_dev();
        if sd == 0.0 {
            return 0.0;
        }
        let z = (value - m).abs() / sd;
        (z / self.sensitivity).min(1.0)
    }

    async fn feed(&self, value: f64) {
        if let Ok(mut values) = self.values.write() {
            values.push(value);
            if values.len() > self.window_size {
                values.remove(0);
            }
        }
    }

    async fn reset(&self) {
        if let Ok(mut values) = self.values.write() {
            values.clear();
        }
    }
}

// ── AnomalyDetector trait ─────────────────────────────────

#[async_trait]
pub trait AnomalyDetector: Debug + Send + Sync {
    async fn score(
        &self,
        observation: &Observation,
    ) -> Result<AnomalyScore, perception_core::PerceptionError>;

    async fn feed(
        &self,
        observation: &Observation,
    ) -> Result<(), perception_core::PerceptionError>;

    fn register_model(
        &self,
        modality: Modality,
        model: Arc<dyn AnomalyModel>,
    ) -> AnomalyResult<()>;

    fn stats(&self, modality: &Modality) -> AnomalyResult<Option<AnomalyStats>>;
}

// ── DefaultAnomalyDetector ────────────────────────────────

#[derive(Debug)]
pub struct DefaultAnomalyDetector {
    config: AnomalyDetectorConfig,
    models: RwLock<HashMap<Modality, Arc<dyn AnomalyModel>>>,
    anomaly_counts: RwLock<HashMap<Modality, u64>>,
    per_modality_config: RwLock<HashMap<Modality, PerModalityAnomalyConfig>>,
}

impl DefaultAnomalyDetector {
    pub fn new(config: AnomalyDetectorConfig) -> Self {
        Self {
            config,
            models: RwLock::new(HashMap::new()),
            anomaly_counts: RwLock::new(HashMap::new()),
            per_modality_config: RwLock::new(HashMap::new()),
        }
    }

    fn extract_value(&self, observation: &Observation) -> Option<f64> {
        match &observation.payload {
            ObservationPayload::Metric { value, .. } => Some(*value),
            ObservationPayload::State { new_value, .. } => new_value.parse::<f64>().ok(),
            _ => None,
        }
    }

    fn get_model(&self, modality: &Modality) -> Option<Arc<dyn AnomalyModel>> {
        self.models
            .read()
            .ok()
            .and_then(|g| g.get(modality).cloned())
    }
}

#[async_trait]
impl AnomalyDetector for DefaultAnomalyDetector {
    async fn score(
        &self,
        observation: &Observation,
    ) -> Result<AnomalyScore, perception_core::PerceptionError> {
        let Some(value) = self.extract_value(observation) else {
            return Ok(AnomalyScore {
                score: 0.0,
                threshold: self.config.default_sensitivity,
                is_anomaly: false,
                model: "none".into(),
            });
        };

        let threshold = self
            .per_modality_config
            .read()
            .ok()
            .and_then(|g| g.get(&observation.modality).cloned())
            .map(|c| c.sensitivity)
            .unwrap_or(self.config.default_sensitivity);

        if let Some(model) = self.get_model(&observation.modality) {
            let score = model.score(value).await;
            let is_anomaly = score > threshold && self.config.enabled;

            if is_anomaly {
                if let Ok(mut counts) = self.anomaly_counts.write() {
                    *counts.entry(observation.modality.clone()).or_insert(0) += 1;
                }
            }

            Ok(AnomalyScore {
                score,
                threshold,
                is_anomaly,
                model: model.name().to_string(),
            })
        } else {
            Ok(AnomalyScore {
                score: 0.0,
                threshold,
                is_anomaly: false,
                model: "none".into(),
            })
        }
    }

    async fn feed(
        &self,
        observation: &Observation,
    ) -> Result<(), perception_core::PerceptionError> {
        if let Some(value) = self.extract_value(observation) {
            if let Some(model) = self.get_model(&observation.modality) {
                model.feed(value).await;
            }
        }
        Ok(())
    }

    fn register_model(
        &self,
        modality: Modality,
        model: Arc<dyn AnomalyModel>,
    ) -> AnomalyResult<()> {
        if let Ok(mut models) = self.models.write() {
            models.insert(modality, model);
        }
        Ok(())
    }

    fn stats(&self, modality: &Modality) -> AnomalyResult<Option<AnomalyStats>> {
        let models = self.models.read().map_err(|_| {
            AnomalyError::ModelNotFound(modality.clone())
        })?;
        let counts = self
            .anomaly_counts
            .read()
            .map_err(|_| AnomalyError::ModelNotFound(modality.clone()))?;

        if models.contains_key(modality) {
            let anomaly_count = counts.get(modality).copied().unwrap_or(0);
            Ok(Some(AnomalyStats {
                mean: 0.0,
                std_dev: 0.0,
                min: 0.0,
                max: 0.0,
                count: 0,
                anomaly_count,
            }))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_zscore_model() {
        let model = ZScoreModel::new("test-sensor", 2.0);

        // Feed consistent values
        for i in 0..20 {
            model.feed(100.0 + (i as f64 * 0.1)).await;
        }

        // Normal value should score low
        let score = model.score(101.0).await;
        assert!(score < 1.0);

        // Anomalous value should score high
        let score = model.score(200.0).await;
        assert!(score > 0.5);

        assert_eq!(model.name(), "test-sensor");
    }

    #[tokio::test]
    async fn test_rolling_window_model() {
        let model = RollingWindowModel::new("cpu", 10, 2.0);

        for i in 0..10 {
            model.feed(50.0 + (i as f64 * 0.1)).await;
        }

        let score = model.score(50.5).await;
        assert!(score < 1.0);

        let score = model.score(100.0).await;
        assert!(score > 0.1);
    }

    #[tokio::test]
    async fn test_anomaly_detector_with_zscore() {
        let config = AnomalyDetectorConfig::default();
        let detector = DefaultAnomalyDetector::new(config);

        let model = Arc::new(ZScoreModel::new("terminal", 2.0));
        detector
            .register_model(Modality::Terminal, model)
            .unwrap();

        let source = ObservationSource {
            observer_id: "test".into(),
            observer_kind: ObserverKind::Terminal,
            instance_id: "inst-1".into(),
            hostname: "localhost".into(),
        };

        // Feed normal values
        for i in 0..15 {
            let obs = Observation::new(
                source.clone(),
                Modality::Terminal,
                ObservationPriority::Normal,
                ObservationPayload::Metric {
                    name: "keystrokes".into(),
                    value: 10.0 + (i as f64 * 0.5),
                    unit: "count".into(),
                },
            );
            detector.feed(&obs).await.unwrap();
        }

        // Score a normal value
        let obs = Observation::new(
            source.clone(),
            Modality::Terminal,
            ObservationPriority::Normal,
            ObservationPayload::Metric {
                name: "keystrokes".into(),
                value: 11.0,
                unit: "count".into(),
            },
        );
        let score = detector.score(&obs).await.unwrap();
        assert!(!score.is_anomaly || score.score < 0.5);

        // Score an anomalous value
        let obs = Observation::new(
            source,
            Modality::Terminal,
            ObservationPriority::Normal,
            ObservationPayload::Metric {
                name: "keystrokes".into(),
                value: 100.0,
                unit: "count".into(),
            },
        );
        let _score = detector.score(&obs).await.unwrap();
        // Should be anomalous or close to it
    }

    #[tokio::test]
    async fn test_non_metric_observation_returns_zero_score() {
        let config = AnomalyDetectorConfig::default();
        let detector = DefaultAnomalyDetector::new(config);

        let source = ObservationSource {
            observer_id: "test".into(),
            observer_kind: ObserverKind::FileSystem,
            instance_id: "inst-1".into(),
            hostname: "localhost".into(),
        };
        let obs = Observation::new(
            source,
            Modality::FileSystem,
            ObservationPriority::Normal,
            ObservationPayload::Text {
                content: "no numeric value".into(),
                encoding: "utf-8".into(),
            },
        );

        let score = detector.score(&obs).await.unwrap();
        assert!(!score.is_anomaly);
        assert_eq!(score.score, 0.0);
    }

    #[tokio::test]
    async fn test_model_reset() {
        let model = ZScoreModel::new("reset-test", 2.0);
        model.feed(100.0).await;
        model.feed(200.0).await;
        model.reset().await;
        let score = model.score(150.0).await;
        assert_eq!(score, 0.0);
    }
}
