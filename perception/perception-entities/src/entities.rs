use async_trait::async_trait;
use perception_core::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::RwLock;

pub type EntityResult<T> = Result<T, EntityError>;

#[derive(Debug, thiserror::Error)]
pub enum EntityError {
    #[error("entity pattern not found: {0}")]
    PatternNotFound(String),
    #[error("resolution failed: {entity_type} = {raw_value}")]
    ResolutionFailed {
        entity_type: String,
        raw_value: String,
    },
    #[error("knowledge graph unavailable: {0}")]
    KnowledgeGraphUnavailable(String),
    #[error("cache error: {0}")]
    CacheError(String),
}

// ── Entity types ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityRef {
    pub entity_id: String,
    pub entity_kind: EntityKind,
    pub label: String,
    pub confidence: Confidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EntityKind {
    Process,
    File,
    User,
    Session,
    NetworkConnection,
    Device,
    Service,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionPattern {
    pub name: String,
    pub entity_kind: EntityKind,
    pub pattern: String,
    pub modality: Modality,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityResolution {
    pub raw_value: String,
    pub resolved: Option<EntityRef>,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityExtractorConfig {
    pub cache_ttl_seconds: u64,
    pub max_patterns: usize,
    pub enable_knowledge_graph: bool,
}

impl Default for EntityExtractorConfig {
    fn default() -> Self {
        Self {
            cache_ttl_seconds: 300,
            max_patterns: 1000,
            enable_knowledge_graph: true,
        }
    }
}

// ── EntityExtractor trait ─────────────────────────────────

#[async_trait]
pub trait EntityExtractor: Debug + Send + Sync {
    async fn extract(
        &self,
        observation: &Observation,
    ) -> Result<Vec<EntityRef>, perception_core::PerceptionError>;

    async fn resolve(
        &self,
        kind: EntityKind,
        raw: &str,
    ) -> Result<Option<EntityRef>, perception_core::PerceptionError>;

    fn register_pattern(&self, pattern: ExtractionPattern) -> EntityResult<()>;

    async fn warm_cache(&self) -> Result<(), perception_core::PerceptionError>;
}

// ── EntityResolver trait ──────────────────────────────────

#[async_trait]
pub trait EntityResolver: Debug + Send + Sync {
    async fn resolve(
        &self,
        kind: EntityKind,
        raw: &str,
    ) -> Result<Option<EntityRef>, perception_core::PerceptionError>;

    async fn resolve_batch(
        &self,
        ids: Vec<(EntityKind, String)>,
    ) -> Result<Vec<Option<EntityRef>>, perception_core::PerceptionError>;

    async fn warm_cache(&self) -> Result<(), perception_core::PerceptionError>;
}

// ── DefaultEntityExtractor ────────────────────────────────

#[derive(Debug)]
pub struct DefaultEntityExtractor {
    #[allow(dead_code)]
    config: EntityExtractorConfig,
    patterns: RwLock<Vec<ExtractionPattern>>,
    cache: RwLock<HashMap<String, EntityRef>>,
}

impl DefaultEntityExtractor {
    pub fn new(config: EntityExtractorConfig) -> Self {
        Self {
            config,
            patterns: RwLock::new(Vec::new()),
            cache: RwLock::new(HashMap::new()),
        }
    }

    fn extract_from_text(&self, text: &str, modality: &Modality) -> Vec<EntityRef> {
        let mut entities = Vec::new();
        let patterns = match self.patterns.read() {
            Ok(p) => p.clone(),
            Err(_) => return entities,
        };

        for pattern in &patterns {
            if &pattern.modality != modality {
                continue;
            }
            if let Ok(re) = regex::Regex::new(&pattern.pattern) {
                for cap in re.captures_iter(text) {
                    if let Some(m) = cap.get(1) {
                        let raw = m.as_str().to_string();
                        let cache_key = format!("{:?}:{}", pattern.entity_kind, raw);

                        // Check cache
                        if let Ok(cache) = self.cache.read() {
                            if let Some(cached) = cache.get(&cache_key) {
                                entities.push(cached.clone());
                                continue;
                            }
                        }

                        let entity_ref = EntityRef {
                            entity_id: raw.clone(),
                            entity_kind: pattern.entity_kind.clone(),
                            label: format!("{}:{}", pattern.name, raw),
                            confidence: Confidence::new(0.7),
                        };

                        // Update cache
                        if let Ok(mut cache) = self.cache.write() {
                            cache.insert(cache_key, entity_ref.clone());
                        }

                        entities.push(entity_ref);
                    }
                }
            }
        }
        entities
    }
}

#[async_trait]
impl EntityExtractor for DefaultEntityExtractor {
    async fn extract(
        &self,
        observation: &Observation,
    ) -> Result<Vec<EntityRef>, perception_core::PerceptionError> {
        let entities = match &observation.payload {
            ObservationPayload::Text { content, .. } => {
                self.extract_from_text(content, &observation.modality)
            }
            ObservationPayload::Structured { fields } => {
                let text = serde_json::to_string(fields).unwrap_or_default();
                self.extract_from_text(&text, &observation.modality)
            }
            ObservationPayload::Event { event_type, .. } => {
                self.extract_from_text(event_type, &observation.modality)
            }
            _ => Vec::new(),
        };
        Ok(entities)
    }

    async fn resolve(
        &self,
        kind: EntityKind,
        raw: &str,
    ) -> Result<Option<EntityRef>, perception_core::PerceptionError> {
        let cache_key = format!("{:?}:{}", kind, raw);
        if let Ok(cache) = self.cache.read() {
            if let Some(cached) = cache.get(&cache_key) {
                return Ok(Some(cached.clone()));
            }
        }
        Ok(Some(EntityRef {
            entity_id: raw.to_string(),
            entity_kind: kind,
            label: format!("resolved:{}", raw),
            confidence: Confidence::new(0.5),
        }))
    }

    fn register_pattern(&self, pattern: ExtractionPattern) -> EntityResult<()> {
        let mut patterns = self
            .patterns
            .write()
            .map_err(|_| EntityError::PatternNotFound("lock poisoned".into()))?;
        // Validate regex
        regex::Regex::new(&pattern.pattern).map_err(|e| {
            EntityError::PatternNotFound(format!("invalid regex: {}", e))
        })?;
        patterns.push(pattern);
        Ok(())
    }

    async fn warm_cache(&self) -> Result<(), perception_core::PerceptionError> {
        // Warm cache by pre-loading common entity patterns
        // Default implementation is a no-op
        Ok(())
    }
}

// ── DefaultEntityResolver ─────────────────────────────────

#[derive(Debug)]
pub struct DefaultEntityResolver {
    cache: RwLock<HashMap<String, EntityRef>>,
}

impl DefaultEntityResolver {
    pub fn new() -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for DefaultEntityResolver {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl EntityResolver for DefaultEntityResolver {
    async fn resolve(
        &self,
        kind: EntityKind,
        raw: &str,
    ) -> Result<Option<EntityRef>, perception_core::PerceptionError> {
        let cache_key = format!("{:?}:{}", kind, raw);
        if let Ok(cache) = self.cache.read() {
            if let Some(cached) = cache.get(&cache_key) {
                return Ok(Some(cached.clone()));
            }
        }
        // Simulate knowledge graph lookup
        let entity = EntityRef {
            entity_id: raw.to_string(),
            entity_kind: kind,
            label: format!("kg:{}", raw),
            confidence: Confidence::new(0.6),
        };
        if let Ok(mut cache) = self.cache.write() {
            cache.insert(cache_key, entity.clone());
        }
        Ok(Some(entity))
    }

    async fn resolve_batch(
        &self,
        ids: Vec<(EntityKind, String)>,
    ) -> Result<Vec<Option<EntityRef>>, perception_core::PerceptionError> {
        let mut results = Vec::with_capacity(ids.len());
        for (kind, raw) in ids {
            results.push(self.resolve(kind, &raw).await?);
        }
        Ok(results)
    }

    async fn warm_cache(&self) -> Result<(), perception_core::PerceptionError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use perception_core::{Observation, ObservationPayload, ObservationPriority, ObservationSource, ObserverKind, Modality};

    #[tokio::test]
    async fn test_entity_extraction_with_pattern() {
        let config = EntityExtractorConfig::default();
        let extractor = DefaultEntityExtractor::new(config);

        let pattern = ExtractionPattern {
            name: "pid".into(),
            entity_kind: EntityKind::Process,
            pattern: r"PID (\d+)".into(),
            modality: Modality::Terminal,
        };
        extractor.register_pattern(pattern).unwrap();

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
                content: "Process PID 1234 started".into(),
                encoding: "utf-8".into(),
            },
        );

        let entities = extractor.extract(&obs).await.unwrap();
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].entity_id, "1234");
        assert_eq!(entities[0].entity_kind, EntityKind::Process);
    }

    #[tokio::test]
    async fn test_entity_extraction_no_match() {
        let config = EntityExtractorConfig::default();
        let extractor = DefaultEntityExtractor::new(config);

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
                content: "no entities here".into(),
                encoding: "utf-8".into(),
            },
        );

        let entities = extractor.extract(&obs).await.unwrap();
        assert!(entities.is_empty());
    }

    #[tokio::test]
    async fn test_entity_resolver() {
        let resolver = DefaultEntityResolver::new();

        let result = resolver
            .resolve(EntityKind::Process, "5678")
            .await
            .unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().entity_id, "5678");
    }

    #[tokio::test]
    async fn test_batch_resolution() {
        let resolver = DefaultEntityResolver::new();

        let ids = vec![
            (EntityKind::Process, "100".into()),
            (EntityKind::File, "/tmp/test".into()),
        ];
        let results = resolver.resolve_batch(ids).await.unwrap();
        assert_eq!(results.len(), 2);
        assert!(results[0].is_some());
        assert!(results[1].is_some());
    }
}
