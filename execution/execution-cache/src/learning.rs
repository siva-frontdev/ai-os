use async_trait::async_trait;
use execution_core::*;
use memory_core::Timestamp;
use std::collections::HashMap;
use tokio::sync::RwLock;
use tracing::debug;

#[derive(Debug)]
pub struct DefaultLearningEngine {
    learned: RwLock<HashMap<String, LearnedCapability>>,
}

impl DefaultLearningEngine {
    pub fn new() -> Self {
        Self {
            learned: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for DefaultLearningEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LearningEngine for DefaultLearningEngine {
    async fn record_execution(
        &self,
        capability_id: &str,
        success: bool,
        duration_ms: u64,
        failure_mode: Option<String>,
    ) -> execution_core::ExecutionResult<()> {
        let mut learned = self.learned.write().await;
        if let Some(entry) = learned.get_mut(capability_id) {
            entry.execution_count += 1;
            if success {
                entry.success_count += 1;
            } else {
                entry.failure_count += 1;
                if let Some(mode) = failure_mode {
                    if !entry.failure_modes.contains(&mode) {
                        entry.failure_modes.push(mode);
                    }
                }
            }
            let total = entry.execution_count as f64;
            entry.trust_score = entry.success_count as f64 / total;
            entry.avg_duration_ms = ((entry.avg_duration_ms as f64 * (total - 1.0)
                + duration_ms as f64)
                / total) as u64;
            entry.last_execution = Some(Timestamp::now());
        } else {
            let mut entry = LearnedCapability {
                capability_id: capability_id.into(),
                capability_name: capability_id.into(),
                origin: CapabilityOrigin::Learned,
                provider_metadata: ProviderMetadata::new("learned", "learned", "learned"),
                execution_count: 1,
                success_count: if success { 1 } else { 0 },
                failure_count: if success { 0 } else { 1 },
                avg_duration_ms: duration_ms,
                trust_score: if success { 1.0 } else { 0.0 },
                failure_modes: Vec::new(),
                compatible: success,
                last_execution: Some(Timestamp::now()),
                created_at: Timestamp::now(),
            };
            if let Some(mode) = failure_mode {
                entry.failure_modes.push(mode);
            }
            learned.insert(capability_id.into(), entry);
        }
        debug!(capability = %capability_id, success, "execution recorded");
        Ok(())
    }

    async fn get_learned(
        &self,
        capability_id: &str,
    ) -> execution_core::ExecutionResult<Option<LearnedCapability>> {
        let learned = self.learned.read().await;
        Ok(learned.get(capability_id).cloned())
    }

    async fn update_trust_score(
        &self,
        capability_id: &str,
        delta: f64,
    ) -> execution_core::ExecutionResult<()> {
        let mut learned = self.learned.write().await;
        if let Some(entry) = learned.get_mut(capability_id) {
            entry.trust_score = (entry.trust_score + delta).clamp(0.0, 1.0);
        }
        Ok(())
    }

    async fn list_learned(&self) -> execution_core::ExecutionResult<Vec<LearnedCapability>> {
        let learned = self.learned.read().await;
        let mut list: Vec<_> = learned.values().cloned().collect();
        list.sort_by(|a, b| {
            b.trust_score
                .partial_cmp(&a.trust_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        Ok(list)
    }

    async fn get_top_performers(
        &self,
        limit: usize,
    ) -> execution_core::ExecutionResult<Vec<LearnedCapability>> {
        let mut list = self.list_learned().await?;
        list.truncate(limit);
        Ok(list)
    }

    async fn get_troublesome(
        &self,
        limit: usize,
    ) -> execution_core::ExecutionResult<Vec<LearnedCapability>> {
        let learned = self.learned.read().await;
        let mut list: Vec<_> = learned.values().cloned().collect();
        list.sort_by(|a, b| {
            b.failure_count.cmp(&a.failure_count).then_with(|| {
                a.trust_score
                    .partial_cmp(&b.trust_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
        });
        list.truncate(limit);
        Ok(list)
    }

    async fn forget(&self, capability_id: &str) -> execution_core::ExecutionResult<()> {
        let mut learned = self.learned.write().await;
        learned.remove(capability_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_record_and_get_learned() {
        let engine = DefaultLearningEngine::new();
        engine
            .record_execution("test.cap", true, 100, None)
            .await
            .unwrap();
        let entry = engine.get_learned("test.cap").await.unwrap().unwrap();
        assert_eq!(entry.execution_count, 1);
        assert_eq!(entry.success_count, 1);
        assert!((entry.trust_score - 1.0).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_record_failure_updates_stats() {
        let engine = DefaultLearningEngine::new();
        engine
            .record_execution("test.cap", true, 100, None)
            .await
            .unwrap();
        engine
            .record_execution("test.cap", false, 200, Some("timeout".to_string()))
            .await
            .unwrap();
        let entry = engine.get_learned("test.cap").await.unwrap().unwrap();
        assert_eq!(entry.execution_count, 2);
        assert_eq!(entry.success_count, 1);
        assert_eq!(entry.failure_count, 1);
        assert!((entry.trust_score - 0.5).abs() < 0.01);
        assert!(entry.failure_modes.contains(&"timeout".to_string()));
    }

    #[tokio::test]
    async fn test_update_trust_score() {
        let engine = DefaultLearningEngine::new();
        engine
            .record_execution("test.cap", true, 100, None)
            .await
            .unwrap();
        engine.update_trust_score("test.cap", -0.2).await.unwrap();
        let entry = engine.get_learned("test.cap").await.unwrap().unwrap();
        assert!((entry.trust_score - 0.8).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_top_performers() {
        let engine = DefaultLearningEngine::new();
        engine
            .record_execution("good", true, 10, None)
            .await
            .unwrap();
        engine
            .record_execution("bad", false, 100, Some("error".to_string()))
            .await
            .unwrap();
        let top = engine.get_top_performers(1).await.unwrap();
        assert_eq!(top.len(), 1);
        assert_eq!(top[0].capability_id, "good");
    }

    #[tokio::test]
    async fn test_troublesome() {
        let engine = DefaultLearningEngine::new();
        engine
            .record_execution("good", true, 10, None)
            .await
            .unwrap();
        engine
            .record_execution("bad", false, 100, Some("error".to_string()))
            .await
            .unwrap();
        let trouble = engine.get_troublesome(1).await.unwrap();
        assert_eq!(trouble.len(), 1);
        assert_eq!(trouble[0].capability_id, "bad");
    }

    #[tokio::test]
    async fn test_forget() {
        let engine = DefaultLearningEngine::new();
        engine
            .record_execution("test.cap", true, 100, None)
            .await
            .unwrap();
        engine.forget("test.cap").await.unwrap();
        assert!(engine.get_learned("test.cap").await.unwrap().is_none());
    }
}
