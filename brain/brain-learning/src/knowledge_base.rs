use crate::errors::{LearningError, LearningResult};
use crate::types::{ConsolidationReport, Knowledge, Pattern};
use brain_core::types::Confidence;
use brain_reflection::types::MistakeCategory;
use memory_core::Timestamp;
use std::collections::HashMap;
use std::sync::RwLock;

pub struct KnowledgeBase {
    store: RwLock<HashMap<String, Knowledge>>,
    version_counter: RwLock<u32>,
}

impl KnowledgeBase {
    pub fn new() -> Self {
        Self {
            store: RwLock::new(HashMap::new()),
            version_counter: RwLock::new(1),
        }
    }

    pub fn consolidate(&self, patterns: Vec<Pattern>) -> LearningResult<ConsolidationReport> {
        let lesson_count = patterns.iter().flat_map(|p| p.lessons.iter()).count() as u32;

        let merged_patterns = self.merge_patterns(&patterns);

        let avg_conf = if patterns.is_empty() {
            0.0
        } else {
            patterns.iter().map(|p| p.confidence.raw()).sum::<f32>() / patterns.len() as f32
        };

        let mut version = self.version_counter.write().map_err(|e| LearningError::Internal(e.to_string()))?;
        let current_version = *version;
        *version += 1;

        let knowledge = Knowledge {
            id: format!("knowledge-{}", current_version),
            summary: format!("consolidated learning from {} patterns across {} lessons", patterns.len(), lesson_count),
            patterns: patterns.clone(),
            confidence: Confidence::new(avg_conf),
            created_at: Timestamp::now(),
            updated_at: Timestamp::now(),
            version: current_version,
        };

        let report = ConsolidationReport {
            knowledge: knowledge.clone(),
            patterns_found: patterns.clone(),
            pattern_count: patterns.len() as u32,
            lesson_count,
            merged_patterns,
        };

        let mut store = self.store.write().map_err(|e| LearningError::Internal(e.to_string()))?;
        store.insert(knowledge.id.clone(), knowledge);

        Ok(report)
    }

    fn merge_patterns(&self, patterns: &[Pattern]) -> Vec<String> {
        let mut merged = Vec::new();
        let mut seen: HashMap<MistakeCategory, usize> = HashMap::new();
        for pattern in patterns {
            use std::collections::hash_map::Entry;
            match seen.entry(pattern.category) {
                Entry::Occupied(_) => {
                    merged.push(format!("merged duplicate: {}", pattern.id));
                }
                Entry::Vacant(e) => {
                    e.insert(0);
                }
            }
        }
        merged
    }

    pub fn get_knowledge(&self, id: &str) -> LearningResult<Knowledge> {
        let store = self.store.read().map_err(|e| LearningError::Internal(e.to_string()))?;
        store.get(id).cloned().ok_or_else(|| LearningError::ConsolidationFailed(format!("knowledge {} not found", id)))
    }

    pub fn list_knowledge(&self) -> LearningResult<Vec<Knowledge>> {
        let store = self.store.read().map_err(|e| LearningError::Internal(e.to_string()))?;
        let mut all: Vec<Knowledge> = store.values().cloned().collect();
        all.sort_by_key(|b| std::cmp::Reverse(b.version));
        Ok(all)
    }
}

impl Default for KnowledgeBase {
    fn default() -> Self { Self::new() }
}
