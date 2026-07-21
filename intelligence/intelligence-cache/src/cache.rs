use super::error::*;
use async_trait::async_trait;
use dashmap::DashMap;
use intelligence_core::error::ModelResult;
use intelligence_core::traits::ResponseCache;
use intelligence_core::types::{CacheKey, CacheStats, ModelId, ModelResponse};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::Duration;

fn cache_result_to_model<T>(res: CacheResult<T>) -> ModelResult<T> {
  res.map_err(|e| intelligence_core::error::ModelError::Io(e.to_string()))
}

#[derive(Debug, Clone)]
pub struct CacheEntry {
  pub response: ModelResponse,
  pub inserted_at: std::time::Instant,
  pub ttl: Duration,
  pub hits: u64,
}

impl CacheEntry {
  pub fn expired(&self) -> bool { self.inserted_at.elapsed() > self.ttl }
}

type EntryKey = String;

#[derive(Debug, Default)]
pub struct DefaultResponseCache {
  entries: DashMap<EntryKey, CacheEntry>,
  by_model: DashMap<String, Vec<EntryKey>>,
}

impl DefaultResponseCache {
  pub fn new() -> Self { Self::default() }

  fn sha256_key(key: &CacheKey) -> String {
    let mut hasher = DefaultHasher::new();
    key.input_hash.hash(&mut hasher);
    key.parameters_hash.hash(&mut hasher);
    key.model_id.0.hash(&mut hasher);
    format!("{:x}", hasher.finish())
  }

  pub fn inner_get(&self, key: &CacheKey) -> CacheResult<Option<ModelResponse>> {
    let digest = Self::sha256_key(key);
    let Some(mut entry) = self.entries.get_mut(&digest) else { return Ok(None); };
    if entry.expired() { return Ok(None); }
    entry.hits += 1;
    Ok(Some(entry.response.clone()))
  }

  pub fn inner_put(&self, key: CacheKey, response: ModelResponse, ttl: Duration) -> CacheResult<()> {
    let digest = Self::sha256_key(&key);
    let model_key = key.model_id.to_string();
    let entry = CacheEntry {
      response,
      inserted_at: std::time::Instant::now(),
      ttl,
      hits: 0,
    };
    let mut keys = self.by_model.entry(model_key.clone()).or_default();
    keys.push(digest.clone());
    self.entries.insert(digest, entry);
    Ok(())
  }

  pub fn inner_stats(&self) -> CacheStats {
    let mut stats = CacheStats::default();
    for entry in self.entries.iter() { stats.hits += entry.hits; stats.entry_count += 1; }
    stats
  }
}

#[async_trait]
impl ResponseCache for DefaultResponseCache {
  async fn get(&self, key: &CacheKey) -> ModelResult<Option<ModelResponse>> {
    cache_result_to_model(self.inner_get(key))
  }

  async fn put(&self, key: CacheKey, response: ModelResponse, ttl: Duration) -> ModelResult<()> {
    cache_result_to_model(self.inner_put(key, response, ttl))
  }

  async fn invalidate(&self, model_id: &ModelId) -> ModelResult<u64> {
    let model_key = model_id.to_string();
    let Some(keys) = self.by_model.remove(&model_key).map(|(_, v)| v) else { return Ok(0); };
    let mut count = 0u64;
    for k in keys { self.entries.remove(&k); count += 1; }
    Ok(count)
  }

  async fn clear(&self) -> ModelResult<()> { self.entries.clear(); Ok(()) }

  fn stats(&self) -> CacheStats { self.inner_stats() }
}

#[cfg(test)]
mod tests {
  use super::*;
  use intelligence_core::types::{CapabilityKind, ModelId, RequestId, TokenUsage};

  #[test]
  fn put_and_get() {
    let cache = DefaultResponseCache::new();
    let key = CacheKey { model_id: ModelId::new(), capability: CapabilityKind::Chat, input_hash: "abc".into(), parameters_hash: "def".into() };
    let resp = ModelResponse { request_id: RequestId::new(), model_id: key.model_id, content: String::new(), usage: TokenUsage::default(), finished: true, finish_reason: None };
    cache.inner_put(key.clone(), resp.clone(), Duration::from_secs(60)).unwrap();
    let got = cache.inner_get(&key).unwrap();
    assert!(got.is_some());
  }

  #[tokio::test]
  async fn response_cache_put_and_get() {
    let cache = DefaultResponseCache::new();
    let key = CacheKey { model_id: ModelId::new(), capability: CapabilityKind::Chat, input_hash: "abc".into(), parameters_hash: "def".into() };
    let resp = ModelResponse { request_id: RequestId::new(), model_id: key.model_id, content: String::new(), usage: TokenUsage::default(), finished: true, finish_reason: None };
    cache.put(key.clone(), resp.clone(), Duration::from_secs(60)).await.unwrap();
    let got = cache.get(&key).await.unwrap();
    assert!(got.is_some());
    assert_eq!(got.unwrap().content, String::new());
  }

  #[tokio::test]
  async fn response_cache_invalidate_returns_count() {
    let cache = DefaultResponseCache::new();
    let key = CacheKey { model_id: ModelId::new(), capability: CapabilityKind::Chat, input_hash: "x".into(), parameters_hash: "y".into() };
    let resp = ModelResponse { request_id: RequestId::new(), model_id: key.model_id, content: "hi".into(), usage: TokenUsage::default(), finished: true, finish_reason: None };
    cache.put(key.clone(), resp, Duration::from_secs(60)).await.unwrap();
    let count = cache.invalidate(&key.model_id).await.unwrap();
    assert_eq!(count, 1);
  }

  #[tokio::test]
  async fn response_cache_clear() {
    let cache = DefaultResponseCache::new();
    let key = CacheKey { model_id: ModelId::new(), capability: CapabilityKind::Chat, input_hash: "1".into(), parameters_hash: "2".into() };
    let resp = ModelResponse { request_id: RequestId::new(), model_id: key.model_id, content: "data".into(), usage: TokenUsage::default(), finished: true, finish_reason: None };
    cache.put(key, resp, Duration::from_secs(60)).await.unwrap();
    cache.clear().await.unwrap();
    let stats = cache.stats();
    assert_eq!(stats.entry_count, 0);
  }
}
