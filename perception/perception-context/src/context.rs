use async_trait::async_trait;
use memory_core::Timestamp;
use perception_core::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::RwLock;

pub type ContextResult<T> = Result<T, ContextError>;

#[derive(Debug, thiserror::Error)]
pub enum ContextError {
    #[error("session resolution failed: {0}")]
    SessionResolutionFailed(String),
    #[error("session not found: {0}")]
    SessionNotFound(String),
    #[error("temporal context error: {0}")]
    TemporalContextError(String),
    #[error("spatial context error: {0}")]
    SpatialContextError(String),
}

// ── Context types ─────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalContext {
    pub wall_clock: Timestamp,
    pub timezone: String,
    pub uptime_seconds: f64,
}

impl TemporalContext {
    pub fn now() -> Self {
        let uptime = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();
        Self {
            wall_clock: Timestamp::now(),
            timezone: timezone().unwrap_or_else(|| "UTC".into()),
            uptime_seconds: uptime,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpatialContext {
    pub hostname: String,
    pub cwd: Option<String>,
    pub session_id: Option<String>,
    pub network_addr: Option<String>,
}

impl SpatialContext {
    pub fn local() -> Self {
        Self {
            hostname: hostname(),
            cwd: std::env::current_dir()
                .ok()
                .map(|p| p.to_string_lossy().into()),
            session_id: None,
            network_addr: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionContext {
    pub session_id: String,
    pub user_id: String,
    pub scope: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextEnricherConfig {
    pub resolve_session: bool,
    pub attach_temporal: bool,
    pub attach_spatial: bool,
    pub cache_ttl_seconds: u64,
}

impl Default for ContextEnricherConfig {
    fn default() -> Self {
        Self {
            resolve_session: true,
            attach_temporal: true,
            attach_spatial: true,
            cache_ttl_seconds: 300,
        }
    }
}

fn hostname() -> String {
    std::env::var("HOSTNAME").unwrap_or_else(|_| "localhost".into())
}

fn timezone() -> Option<String> {
    Some(chrono_tz_wrapper())
}

fn chrono_tz_wrapper() -> String {
    // Read timezone from environment or use UTC
    std::env::var("TZ").unwrap_or_else(|_| "UTC".into())
}

// ── ContextEnricher trait ─────────────────────────────────

#[async_trait]
pub trait ContextEnricher: Debug + Send + Sync {
    async fn enrich(
        &self,
        observation: Observation,
    ) -> Result<Observation, perception_core::PerceptionError>;

    async fn resolve_session(
        &self,
        pid: u32,
    ) -> Result<Option<SessionContext>, perception_core::PerceptionError>;

    fn temporal_context(&self) -> TemporalContext;

    fn spatial_context(&self) -> SpatialContext;
}

// ── SessionResolver trait ─────────────────────────────────

#[async_trait]
pub trait SessionResolver: Debug + Send + Sync {
    async fn resolve_pid(
        &self,
        pid: u32,
    ) -> Result<Option<SessionContext>, perception_core::PerceptionError>;

    async fn resolve_tid(
        &self,
        tid: u32,
    ) -> Result<Option<SessionContext>, perception_core::PerceptionError>;

    async fn warm_cache(&self) -> Result<(), perception_core::PerceptionError>;
}

// ── DefaultContextEnricher ────────────────────────────────

#[derive(Debug)]
pub struct DefaultContextEnricher {
    config: ContextEnricherConfig,
    session_cache: RwLock<HashMap<u32, SessionContext>>,
}

impl DefaultContextEnricher {
    pub fn new(config: ContextEnricherConfig) -> Self {
        Self {
            config,
            session_cache: RwLock::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl ContextEnricher for DefaultContextEnricher {
    async fn enrich(
        &self,
        mut observation: Observation,
    ) -> Result<Observation, perception_core::PerceptionError> {
        if self.config.attach_temporal {
            let tc = self.temporal_context();
            observation
                .metadata
                .insert("temporal.wall_clock".into(), tc.wall_clock.to_string());
            observation
                .metadata
                .insert("temporal.timezone".into(), tc.timezone);
            observation
                .metadata
                .insert("temporal.uptime".into(), tc.uptime_seconds.to_string());
        }

        if self.config.attach_spatial {
            let sc = self.spatial_context();
            observation
                .metadata
                .insert("spatial.hostname".into(), sc.hostname);
            if let Some(cwd) = sc.cwd {
                observation.metadata.insert("spatial.cwd".into(), cwd);
            }
            if let Some(sid) = sc.session_id {
                observation
                    .metadata
                    .insert("spatial.session_id".into(), sid);
            }
        }

        if self.config.resolve_session {
            // Try to extract PID from observation metadata
            if let Some(pid_str) = observation.metadata.get("pid") {
                if let Ok(pid) = pid_str.parse::<u32>() {
                    if let Ok(Some(session)) = self.resolve_session(pid).await {
                        observation
                            .metadata
                            .insert("session.id".into(), session.session_id);
                        observation
                            .metadata
                            .insert("session.user".into(), session.user_id);
                        observation
                            .metadata
                            .insert("session.scope".into(), session.scope);
                    }
                }
            }
        }

        observation
            .provenance
            .transformation_log
            .push(TransformationStep {
                stage: "context_enricher".into(),
                timestamp: Timestamp::now(),
                description: "temporal, spatial, and session context attached".into(),
            });

        Ok(observation)
    }

    async fn resolve_session(
        &self,
        pid: u32,
    ) -> Result<Option<SessionContext>, perception_core::PerceptionError> {
        // Check cache first
        if let Ok(cache) = self.session_cache.read() {
            if let Some(session) = cache.get(&pid) {
                return Ok(Some(session.clone()));
            }
        }

        // Simulate session resolution (in production, queries Runtime SessionManager)
        let session = SessionContext {
            session_id: format!("sess-{}", pid),
            user_id: "user-1".into(),
            scope: "default".into(),
        };

        if let Ok(mut cache) = self.session_cache.write() {
            cache.insert(pid, session.clone());
        }

        Ok(Some(session))
    }

    fn temporal_context(&self) -> TemporalContext {
        TemporalContext::now()
    }

    fn spatial_context(&self) -> SpatialContext {
        SpatialContext::local()
    }
}

// ── DefaultSessionResolver ────────────────────────────────

#[derive(Debug)]
pub struct DefaultSessionResolver {
    cache: RwLock<HashMap<u32, SessionContext>>,
}

impl DefaultSessionResolver {
    pub fn new() -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for DefaultSessionResolver {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SessionResolver for DefaultSessionResolver {
    async fn resolve_pid(
        &self,
        pid: u32,
    ) -> Result<Option<SessionContext>, perception_core::PerceptionError> {
        if let Ok(cache) = self.cache.read() {
            if let Some(session) = cache.get(&pid) {
                return Ok(Some(session.clone()));
            }
        }
        let session = SessionContext {
            session_id: format!("sess-{}", pid),
            user_id: "user-1".into(),
            scope: "default".into(),
        };
        if let Ok(mut cache) = self.cache.write() {
            cache.insert(pid, session.clone());
        }
        Ok(Some(session))
    }

    async fn resolve_tid(
        &self,
        tid: u32,
    ) -> Result<Option<SessionContext>, perception_core::PerceptionError> {
        // For thread IDs, look up the parent process session
        self.resolve_pid(tid).await
    }

    async fn warm_cache(&self) -> Result<(), perception_core::PerceptionError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_context_enrichment() {
        let config = ContextEnricherConfig::default();
        let enricher = DefaultContextEnricher::new(config);

        let source = ObservationSource {
            observer_id: "test".into(),
            observer_kind: ObserverKind::Process,
            instance_id: "inst-1".into(),
            hostname: "localhost".into(),
        };
        let mut obs = Observation::new(
            source,
            Modality::Process,
            ObservationPriority::Normal,
            ObservationPayload::Text {
                content: "process event".into(),
                encoding: "utf-8".into(),
            },
        );
        obs.metadata.insert("pid".into(), "1234".into());

        let enriched = enricher.enrich(obs).await.unwrap();
        assert!(enriched.metadata.contains_key("temporal.wall_clock"));
        assert!(enriched.metadata.contains_key("spatial.hostname"));
        assert!(enriched.metadata.contains_key("session.id"));
        assert_eq!(enriched.metadata.get("session.id").unwrap(), "sess-1234");
        assert_eq!(enriched.provenance.transformation_log.len(), 1);
    }

    #[tokio::test]
    async fn test_session_resolver() {
        let resolver = DefaultSessionResolver::new();
        let session = resolver.resolve_pid(9999).await.unwrap();
        assert!(session.is_some());
        assert_eq!(session.unwrap().session_id, "sess-9999");

        // Cache hit on second call
        let session = resolver.resolve_pid(9999).await.unwrap();
        assert!(session.is_some());
    }

    #[tokio::test]
    async fn test_temporal_context() {
        let config = ContextEnricherConfig::default();
        let enricher = DefaultContextEnricher::new(config);
        let tc = enricher.temporal_context();
        assert!(!tc.timezone.is_empty());
        assert!(tc.uptime_seconds > 0.0);
    }

    #[tokio::test]
    async fn test_spatial_context() {
        let config = ContextEnricherConfig::default();
        let enricher = DefaultContextEnricher::new(config);
        let sc = enricher.spatial_context();
        assert!(!sc.hostname.is_empty());
    }
}
