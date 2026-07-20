use crate::errors::{LearningError, LearningResult};
use crate::types::{LearningSignal, Pattern};
use brain_core::ids::LessonId;
use brain_core::types::Confidence;
use brain_reflection::types::MistakeCategory;
use memory_core::Timestamp;
use std::collections::HashMap;
use std::sync::RwLock;

fn category_as_str(cat: &MistakeCategory) -> &'static str {
    match cat {
        MistakeCategory::Planning => "planning",
        MistakeCategory::Reasoning => "reasoning",
        MistakeCategory::Execution => "execution",
        MistakeCategory::Timing => "timing",
        MistakeCategory::Resource => "resource",
        MistakeCategory::Unknown => "unknown",
    }
}

pub struct PatternRecognizer {
    signals: RwLock<Vec<LearningSignal>>,
}

impl PatternRecognizer {
    pub fn new() -> Self {
        Self {
            signals: RwLock::new(Vec::new()),
        }
    }

    pub fn record_signal(&self, signal: LearningSignal) -> LearningResult<()> {
        let mut signals = self
            .signals
            .write()
            .map_err(|e| LearningError::Internal(e.to_string()))?;
        signals.push(signal);
        Ok(())
    }

    pub fn extract_patterns(&self) -> LearningResult<Vec<Pattern>> {
        let signals = self
            .signals
            .read()
            .map_err(|e| LearningError::Internal(e.to_string()))?;
        if signals.is_empty() {
            return Err(LearningError::NoPatterns("no signals recorded".into()));
        }

        let mut cat_counts: HashMap<MistakeCategory, u32> = HashMap::new();
        let mut cat_lessons: HashMap<MistakeCategory, Vec<LessonId>> = HashMap::new();
        let mut cat_last: HashMap<MistakeCategory, Timestamp> = HashMap::new();
        let mut cat_descs: HashMap<MistakeCategory, Vec<String>> = HashMap::new();

        for signal in signals.iter() {
            *cat_counts.entry(signal.category).or_insert(0) += 1;
            cat_lessons
                .entry(signal.category)
                .or_default()
                .push(signal.lesson_id);
            cat_last.insert(signal.category, signal.timestamp);
            cat_descs
                .entry(signal.category)
                .or_default()
                .push(signal.description.clone());
        }

        let mut patterns = Vec::new();
        for (category, count) in &cat_counts {
            let descs = cat_descs.get(category).unwrap();
            let freq_desc = descs
                .iter()
                .filter(|d| descs.iter().filter(|x| *x == *d).count() > 1)
                .count();
            let confidence = if *count > 5 {
                0.9
            } else if *count > 2 {
                0.7
            } else {
                0.5
            };

            patterns.push(Pattern {
                id: format!("pattern-{}-{}", category_as_str(category), *count),
                category: *category,
                description: format!(
                    "recurring {} patterns (observed {} times, {} frequent)",
                    category_as_str(category),
                    count,
                    freq_desc
                ),
                frequency: *count,
                confidence: Confidence::new(confidence as f32),
                last_observed: *cat_last.get(category).unwrap(),
                lessons: cat_lessons.get(category).unwrap().clone(),
            });
        }

        patterns.sort_by_key(|b| std::cmp::Reverse(b.frequency));
        Ok(patterns)
    }
}

impl Default for PatternRecognizer {
    fn default() -> Self {
        Self::new()
    }
}
