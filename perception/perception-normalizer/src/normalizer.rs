use async_trait::async_trait;
use memory_core::Timestamp;
use perception_core::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;
use std::sync::RwLock;

pub type NormalizerResult<T> = Result<T, NormalizerError>;

#[derive(Debug, thiserror::Error)]
pub enum NormalizerError {
    #[error("schema not found: {0}")]
    SchemaNotFound(String),
    #[error("schema registration failed: {0}")]
    SchemaRegistrationFailed(String),
    #[error("parser not found for format: {0}")]
    ParserNotFound(String),
    #[error("normalization failed: {0}")]
    NormalizationFailed(String),
}

// ── Schema types ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schema {
    pub name: String,
    pub modality: Modality,
    pub fields: Vec<FieldDef>,
    pub max_depth: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldDef {
    pub name: String,
    pub field_type: FieldType,
    pub required: bool,
    pub max_length: Option<usize>,
    pub pattern: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FieldType {
    String,
    Integer,
    Float,
    Boolean,
    Object,
    Array,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<String>,
}

impl ValidationResult {
    pub fn valid() -> Self {
        Self {
            valid: true,
            errors: Vec::new(),
        }
    }

    pub fn invalid(errors: Vec<String>) -> Self {
        Self {
            valid: false,
            errors,
        }
    }
}

// ── Format types ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormatDescriptor {
    pub mime_type: String,
    pub encoding: String,
}

// ── FormatParser trait ────────────────────────────────────

#[async_trait]
pub trait FormatParser: Debug + Send + Sync {
    fn mime_type(&self) -> &str;
    async fn detect(&self, bytes: &[u8]) -> bool;
    async fn parse(&self, bytes: &[u8]) -> Result<ObservationPayload, perception_core::PerceptionError>;
}

// ── FormatDetector trait ──────────────────────────────────

#[async_trait]
pub trait FormatDetectorFn: Debug + Send + Sync {
    async fn detect(&self, bytes: &[u8]) -> bool;
}

#[async_trait]
pub trait FormatDetector: Debug + Send + Sync {
    async fn detect(&self, bytes: &[u8]) -> Result<(String, f64), perception_core::PerceptionError>;
    fn register_detector(&self, mime: &str, detector: Arc<dyn FormatDetectorFn>);
}

// ── SchemaRegistry trait ──────────────────────────────────

#[async_trait]
pub trait SchemaRegistry: Debug + Send + Sync {
    fn register(&self, modality: Modality, schema: Schema) -> NormalizerResult<()>;
    fn get(&self, modality: &Modality) -> Option<Schema>;
    fn unregister(&self, modality: &Modality) -> NormalizerResult<()>;
    fn modalities(&self) -> Vec<Modality>;
}

// ── NormalizerConfig ──────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizerConfig {
    pub default_encoding: String,
    pub max_payload_bytes: u64,
    pub strict_validation: bool,
    pub strip_raw_bytes: bool,
}

impl Default for NormalizerConfig {
    fn default() -> Self {
        Self {
            default_encoding: "utf-8".into(),
            max_payload_bytes: 1_048_576,
            strict_validation: false,
            strip_raw_bytes: true,
        }
    }
}

// ── Normalizer trait ──────────────────────────────────────

#[async_trait]
pub trait Normalizer: Debug + Send + Sync {
    async fn normalize(&self, observation: Observation) -> Result<Observation, perception_core::PerceptionError>;
    async fn normalize_batch(
        &self,
        observations: Vec<Observation>,
    ) -> Result<Vec<Observation>, perception_core::PerceptionError>;
    fn register_schema(&self, modality: Modality, schema: Schema) -> NormalizerResult<()>;
    fn register_parser(&self, mime: &str, parser: Box<dyn FormatParser>);
}

// ── DefaultNormalizer ─────────────────────────────────────

#[derive(Debug)]
pub struct DefaultNormalizer {
    config: NormalizerConfig,
    schemas: RwLock<HashMap<Modality, Schema>>,
    parsers: RwLock<HashMap<String, Box<dyn FormatParser>>>,
    detectors: RwLock<Vec<(String, Arc<dyn FormatDetectorFn>)>>,
}

impl DefaultNormalizer {
    pub fn new(config: NormalizerConfig) -> Self {
        Self {
            config,
            schemas: RwLock::new(HashMap::new()),
            parsers: RwLock::new(HashMap::new()),
            detectors: RwLock::new(Vec::new()),
        }
    }

    fn validate_against_schema(
        &self,
        payload: &ObservationPayload,
        modality: &Modality,
    ) -> Result<(), perception_core::PerceptionError> {
        let schemas = self.schemas.read().map_err(|_| {
            perception_core::PerceptionError::ConfigurationError("schema lock poisoned".into())
        })?;
        let Some(schema) = schemas.get(modality) else {
            return Ok(());
        };
        let structured = match payload {
            ObservationPayload::Structured { fields } => fields,
            _ => return Ok(()),
        };
        for field in &schema.fields {
            if field.required && !structured.contains_key(&field.name) {
                return Err(perception_core::PerceptionError::ValidationFailed {
                    observation_id: ObservationId::default(),
                    reason: format!("missing required field: {}", field.name),
                });
            }
            if let Some(max_len) = field.max_length {
                if let Some(value) = structured.get(&field.name) {
                    if let Some(s) = value.as_str() {
                        if s.len() > max_len {
                            return Err(perception_core::PerceptionError::ValidationFailed {
                                observation_id: ObservationId::default(),
                                reason: format!(
                                    "field {} exceeds max length {}",
                                    field.name, max_len
                                ),
                            });
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

#[async_trait]
impl Normalizer for DefaultNormalizer {
    async fn normalize(
        &self,
        mut observation: Observation,
    ) -> Result<Observation, perception_core::PerceptionError> {
        // Check payload size
        let size_estimate = match &observation.payload {
            ObservationPayload::Text { content, .. } => content.len() as u64,
            ObservationPayload::Binary { content, .. } => content.len() as u64,
            ObservationPayload::Structured { fields } => {
                serde_json::to_string(fields).unwrap_or_default().len() as u64
            }
            ObservationPayload::Event { data, .. } => {
                serde_json::to_string(data).unwrap_or_default().len() as u64
            }
            ObservationPayload::State { .. } => 256,
            ObservationPayload::Metric { .. } => 128,
        };
        if size_estimate > self.config.max_payload_bytes {
            return Err(perception_core::PerceptionError::PayloadTooBig {
                size: size_estimate,
                max: self.config.max_payload_bytes,
            });
        }

        // Validate against schema
        self.validate_against_schema(&observation.payload, &observation.modality)?;

        // Record transformation
        observation.provenance.transformation_log.push(TransformationStep {
            stage: "normalizer".into(),
            timestamp: Timestamp::now(),
            description: "schema validated, format normalized".into(),
        });

        // Optionally strip raw bytes
        if self.config.strip_raw_bytes {
            observation.provenance.raw_bytes = None;
        }

        Ok(observation)
    }

    async fn normalize_batch(
        &self,
        observations: Vec<Observation>,
    ) -> Result<Vec<Observation>, perception_core::PerceptionError> {
        let mut result = Vec::with_capacity(observations.len());
        for obs in observations {
            result.push(self.normalize(obs).await?);
        }
        Ok(result)
    }

    fn register_schema(&self, modality: Modality, schema: Schema) -> NormalizerResult<()> {
        let mut schemas = self
            .schemas
            .write()
            .map_err(|_| NormalizerError::SchemaRegistrationFailed("lock poisoned".into()))?;
        schemas.insert(modality, schema);
        Ok(())
    }

    fn register_parser(&self, mime: &str, parser: Box<dyn FormatParser>) {
        if let Ok(mut parsers) = self.parsers.write() {
            parsers.insert(mime.to_string(), parser);
        }
    }
}

#[async_trait]
impl FormatDetector for DefaultNormalizer {
    async fn detect(&self, bytes: &[u8]) -> Result<(String, f64), perception_core::PerceptionError> {
        // Collect detector references under the lock, then iterate outside it
        let detector_list: Vec<(String, Arc<dyn FormatDetectorFn>)> = self
            .detectors
            .read()
            .map_err(|_| {
                perception_core::PerceptionError::ConfigurationError(
                    "detector lock poisoned".into(),
                )
            })?
            .iter()
            .map(|(mime, det)| (mime.clone(), det.clone()))
            .collect();
        for (mime, detector) in detector_list.iter() {
            if detector.detect(bytes).await {
                return Ok((mime.clone(), 1.0));
            }
        }
        // Fallback: detect JSON
        if bytes.starts_with(b"{") || bytes.starts_with(b"[") {
            return Ok(("application/json".into(), 0.9));
        }
        if bytes.is_ascii() {
            return Ok(("text/plain".into(), 0.8));
        }
        Ok(("application/octet-stream".into(), 0.5))
    }

    fn register_detector(&self, mime: &str, detector: Arc<dyn FormatDetectorFn>) {
        if let Ok(mut detectors) = self.detectors.write() {
            detectors.push((mime.to_string(), detector));
        }
    }
}

// ── InMemorySchemaRegistry ────────────────────────────────

#[derive(Debug, Default)]
pub struct InMemorySchemaRegistry {
    schemas: RwLock<HashMap<Modality, Schema>>,
}

impl InMemorySchemaRegistry {
    pub fn new() -> Self {
        Self {
            schemas: RwLock::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl SchemaRegistry for InMemorySchemaRegistry {
    fn register(&self, modality: Modality, schema: Schema) -> NormalizerResult<()> {
        let mut schemas = self
            .schemas
            .write()
            .map_err(|_| NormalizerError::SchemaRegistrationFailed("lock poisoned".into()))?;
        schemas.insert(modality, schema);
        Ok(())
    }

    fn get(&self, modality: &Modality) -> Option<Schema> {
        self.schemas
            .read()
            .ok()
            .and_then(|s| s.get(modality).cloned())
    }

    fn unregister(&self, modality: &Modality) -> NormalizerResult<()> {
        let mut schemas = self
            .schemas
            .write()
            .map_err(|_| NormalizerError::SchemaNotFound(format!("{:?}", modality)))?;
        schemas.remove(modality);
        Ok(())
    }

    fn modalities(&self) -> Vec<Modality> {
        self.schemas
            .read()
            .map(|s| s.keys().cloned().collect())
            .unwrap_or_default()
    }
}

// ── JSON FormatParser ─────────────────────────────────────

#[derive(Debug)]
pub struct JsonParser;

#[async_trait]
impl FormatParser for JsonParser {
    fn mime_type(&self) -> &str {
        "application/json"
    }

    async fn detect(&self, bytes: &[u8]) -> bool {
        let trimmed = bytes.trim_ascii();
        trimmed.starts_with(b"{") || trimmed.starts_with(b"[")
    }

    async fn parse(
        &self,
        bytes: &[u8],
    ) -> Result<ObservationPayload, perception_core::PerceptionError> {
        let value: serde_json::Value =
            serde_json::from_slice(bytes).map_err(|e| {
                perception_core::PerceptionError::SanitizationFailed(format!(
                    "invalid JSON: {}",
                    e
                ))
            })?;
        match value {
            serde_json::Value::Object(map) => {
                let fields: HashMap<String, serde_json::Value> = map.into_iter().collect();
                Ok(ObservationPayload::Structured { fields })
            }
            _ => {
                let mut fields = serde_json::Map::new();
                fields.insert("value".into(), value);
                Ok(ObservationPayload::Structured {
                    fields: fields.into_iter().collect(),
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_normalizer_accepts_valid_observation() {
        let config = NormalizerConfig::default();
        let normalizer = DefaultNormalizer::new(config);

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
                content: "valid data".into(),
                encoding: "utf-8".into(),
            },
        );
        let result = normalizer.normalize(obs).await;
        assert!(result.is_ok());
        let normalized = result.unwrap();
        assert_eq!(normalized.provenance.transformation_log.len(), 1);
    }

    #[tokio::test]
    async fn test_normalizer_rejects_oversized_payload() {
        let config = NormalizerConfig {
            max_payload_bytes: 10,
            ..Default::default()
        };
        let normalizer = DefaultNormalizer::new(config);

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
                content: "this is way too long for the limit".into(),
                encoding: "utf-8".into(),
            },
        );
        let result = normalizer.normalize(obs).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            perception_core::PerceptionError::PayloadTooBig { .. }
        ));
    }

    #[tokio::test]
    async fn test_schema_registry() {
        let registry = InMemorySchemaRegistry::new();

        let schema = Schema {
            name: "test-schema".into(),
            modality: Modality::Terminal,
            fields: vec![FieldDef {
                name: "command".into(),
                field_type: FieldType::String,
                required: true,
                max_length: Some(100),
                pattern: None,
            }],
            max_depth: 5,
        };

        registry
            .register(Modality::Terminal, schema.clone())
            .unwrap();
        let retrieved = registry.get(&Modality::Terminal);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "test-schema");

        registry.unregister(&Modality::Terminal).unwrap();
        assert!(registry.get(&Modality::Terminal).is_none());
    }

    #[tokio::test]
    async fn test_format_detection() {
        let config = NormalizerConfig::default();
        let detector = DefaultNormalizer::new(config);

        let (mime, confidence) = detector.detect(b"hello world").await.unwrap();
        assert_eq!(mime, "text/plain");
        assert!((confidence - 0.8).abs() < 0.01);

        let (mime, confidence) = detector.detect(b"{\"key\": \"value\"}").await.unwrap();
        assert_eq!(mime, "application/json");
    }

    #[tokio::test]
    async fn test_json_parser() {
        let parser = JsonParser;
        let json = b"{\"name\": \"test\", \"value\": 42}";
        let result = parser.parse(json).await;
        assert!(result.is_ok());
        match result.unwrap() {
            ObservationPayload::Structured { fields } => {
                assert_eq!(fields.get("name").unwrap(), "test");
                assert_eq!(fields.get("value").unwrap(), 42);
            }
            _ => panic!("expected structured payload"),
        }
    }

    #[tokio::test]
    async fn test_validation_against_schema() {
        let config = NormalizerConfig::default();
        let normalizer = DefaultNormalizer::new(config);

        let schema = Schema {
            name: "terminal-input".into(),
            modality: Modality::Terminal,
            fields: vec![FieldDef {
                name: "command".into(),
                field_type: FieldType::String,
                required: true,
                max_length: None,
                pattern: None,
            }],
            max_depth: 5,
        };
        normalizer
            .register_schema(Modality::Terminal, schema)
            .unwrap();

        let source = ObservationSource {
            observer_id: "test".into(),
            observer_kind: ObserverKind::Terminal,
            instance_id: "inst-1".into(),
            hostname: "localhost".into(),
        };

        // Valid payload with required field
        let mut fields = std::collections::HashMap::new();
        fields.insert("command".into(), serde_json::Value::String("ls".into()));
        let obs = Observation::new(
            source.clone(),
            Modality::Terminal,
            ObservationPriority::Normal,
            ObservationPayload::Structured { fields },
        );
        let result = normalizer.normalize(obs).await;
        assert!(result.is_ok(), "valid payload should pass: {:?}", result);
    }

    #[tokio::test]
    async fn test_format_detector_registration() {
        #[derive(Debug)]
        struct TestDetector;
        #[async_trait]
        impl FormatDetectorFn for TestDetector {
            async fn detect(&self, bytes: &[u8]) -> bool {
                bytes.starts_with(b"MAGIC")
            }
        }

        let config = NormalizerConfig::default();
        let detector = DefaultNormalizer::new(config);
        detector.register_detector("application/x-magic", Arc::new(TestDetector));

        let (mime, _) = detector.detect(b"MAGIC data").await.unwrap();
        assert_eq!(mime, "application/x-magic");
    }
}
