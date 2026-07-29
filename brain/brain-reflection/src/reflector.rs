use crate::errors::ReflectionResult;
use crate::types::{
    ActionableInsight, ComparisonResult, Improvement, ImprovementCategory, Lesson, Mistake,
    MistakeCategory, MistakeSeverity, OutcomeDimensions, Reflection, RootCause,
};
use brain_core::ids::{GoalId, LessonId, ReflectionId};
use brain_core::types::Confidence;
use memory_core::Timestamp;

pub struct Reflector;

impl Reflector {
    pub fn new() -> Self {
        Self
    }

    /// Perform multi-dimensional reflection on a goal outcome.
    ///
    /// Analyzes correctness, completeness, efficiency, safety, and
    /// user satisfaction, then generates mistakes, root causes,
    /// improvements, actionable insights, and lessons.
    pub fn reflect(
        &self,
        goal_id: GoalId,
        expected_outcome: &str,
        actual_outcome: &str,
    ) -> ReflectionResult<Reflection> {
        let comparison = self.compare(expected_outcome, actual_outcome)?;
        let mistakes = self.detect_mistakes(&comparison);
        let root_causes = self.analyze_root_causes(&comparison, &mistakes);
        let improvements = self.generate_improvements(&mistakes, &root_causes);
        let insights = self.generate_actionable_insights(goal_id, &mistakes, &root_causes);
        let lessons = self.extract_lessons(goal_id, &mistakes, &improvements, &insights);

        Ok(Reflection {
            reflection_id: ReflectionId::new(),
            goal_id,
            outcome: actual_outcome.into(),
            dimensions: comparison.dimensions.clone(),
            mistakes,
            improvements,
            root_causes,
            actionable_insights: insights,
            lessons,
            confidence: Confidence::new(comparison.dimensions.overall() as f32),
            created_at: Timestamp::now(),
        })
    }

    /// Compare expected vs actual across multiple dimensions.
    pub fn compare(&self, expected: &str, actual: &str) -> ReflectionResult<ComparisonResult> {
        let expected_words: Vec<&str> = expected.split_whitespace().collect();
        let actual_words: Vec<&str> = actual.split_whitespace().collect();

        let common = expected_words
            .iter()
            .filter(|w| actual_words.contains(w))
            .count();
        let total = expected_words.len().max(actual_words.len());
        let match_score = if total == 0 {
            1.0
        } else {
            common as f64 / total as f64
        };

        let mut deviations = Vec::new();
        if match_score < 0.5 {
            deviations.push("significant deviation between expected and actual outcome".into());
        }
        if expected_words.len() != actual_words.len() {
            deviations.push(format!(
                "length mismatch: expected {} words, got {}",
                expected_words.len(),
                actual_words.len()
            ));
        }

        // Compute multi-dimensional scores based on match quality
        let dimensions = OutcomeDimensions {
            correctness: match_score,
            completeness: (match_score * 0.8 + 0.2).min(1.0),
            efficiency: if expected_words.len() >= actual_words.len() {
                (actual_words.len() as f64 / expected_words.len() as f64).min(1.0)
            } else {
                (expected_words.len() as f64 / actual_words.len() as f64).min(1.0)
            },
            safety: 1.0,
            user_satisfaction: match_score * 0.7 + 0.3,
        };

        Ok(ComparisonResult {
            expected: expected.into(),
            actual: actual.into(),
            match_score,
            deviations,
            dimensions,
        })
    }

    /// Detect mistakes across multiple dimensions.
    fn detect_mistakes(&self, comparison: &ComparisonResult) -> Vec<Mistake> {
        let mut mistakes = Vec::new();
        let dim = &comparison.dimensions;

        if dim.correctness < 0.3 {
            mistakes.push(Mistake {
                id: "mistake-correctness-1".into(),
                description: "outcome does not match expected correctness criteria".into(),
                category: MistakeCategory::Execution,
                severity: MistakeSeverity::Critical,
                root_cause: "incorrect execution or misunderstanding of requirements".into(),
            });
        }
        if dim.completeness < 0.4 {
            mistakes.push(Mistake {
                id: "mistake-completeness-1".into(),
                description: "outcome is incomplete".into(),
                category: MistakeCategory::Planning,
                severity: MistakeSeverity::Major,
                root_cause: "insufficient task decomposition or missing steps".into(),
            });
        }
        if dim.efficiency < 0.3 {
            mistakes.push(Mistake {
                id: "mistake-efficiency-1".into(),
                description: "outcome was inefficient".into(),
                category: MistakeCategory::Resource,
                severity: MistakeSeverity::Minor,
                root_cause: "suboptimal approach or redundant operations".into(),
            });
        }
        if dim.safety < 0.5 {
            mistakes.push(Mistake {
                id: "mistake-safety-1".into(),
                description: "safety concerns detected in outcome".into(),
                category: MistakeCategory::Safety,
                severity: MistakeSeverity::Critical,
                root_cause: "missing safety validation in planning".into(),
            });
        }
        if dim.user_satisfaction < 0.3 {
            mistakes.push(Mistake {
                id: "mistake-communication-1".into(),
                description: "user expectations not met".into(),
                category: MistakeCategory::Communication,
                severity: MistakeSeverity::Major,
                root_cause: "insufficient context gathering or requirement clarification".into(),
            });
        }
        if comparison.deviations.len() > 3 {
            mistakes.push(Mistake {
                id: "mistake-deviations-1".into(),
                description: "multiple unexpected deviations".into(),
                category: MistakeCategory::Unknown,
                severity: MistakeSeverity::Major,
                root_cause: "incomplete world model or incorrect assumptions".into(),
            });
        }

        mistakes
    }

    /// Analyze root causes hierarchically.
    fn analyze_root_causes(
        &self,
        _comparison: &ComparisonResult,
        mistakes: &[Mistake],
    ) -> Vec<RootCause> {
        mistakes
            .iter()
            .map(|m| {
                let (primary, factors) = match m.category {
                    MistakeCategory::Planning => (
                        "incomplete goal decomposition".into(),
                        vec![
                            "missing dependency analysis".into(),
                            "insufficient alternatives considered".into(),
                        ],
                    ),
                    MistakeCategory::Execution => (
                        "incorrect tool or method selection".into(),
                        vec![
                            "tool capability mismatch".into(),
                            "missing fallback strategy".into(),
                        ],
                    ),
                    MistakeCategory::Reasoning => (
                        "incorrect inference or assumption".into(),
                        vec!["insufficient evidence".into(), "confirmation bias".into()],
                    ),
                    MistakeCategory::Knowledge => (
                        "missing or outdated knowledge".into(),
                        vec!["world model gap".into(), "unfamiliar domain".into()],
                    ),
                    MistakeCategory::Resource => (
                        "insufficient resources allocated".into(),
                        vec!["budget exceeded".into(), "time constraint too tight".into()],
                    ),
                    MistakeCategory::Safety => (
                        "safety guardrails not triggered".into(),
                        vec![
                            "missing policy check".into(),
                            "oversight in validation".into(),
                        ],
                    ),
                    MistakeCategory::Communication => (
                        "user intent misalignment".into(),
                        vec![
                            "ambiguous requirements".into(),
                            "missing clarification".into(),
                        ],
                    ),
                    MistakeCategory::Timing => (
                        "timing constraint violation".into(),
                        vec!["operation took too long".into(), "deadline not met".into()],
                    ),
                    MistakeCategory::Unknown => (
                        "unexpected failure mode".into(),
                        vec![
                            "no matching pattern in history".into(),
                            "novel error type".into(),
                        ],
                    ),
                };

                RootCause {
                    primary,
                    contributing_factors: factors,
                    category: m.category,
                    severity: m.severity,
                    fix_suggestion: self.suggest_fix(m.category, m.severity),
                }
            })
            .collect()
    }

    fn suggest_fix(&self, category: MistakeCategory, severity: MistakeSeverity) -> String {
        match (category, severity) {
            (MistakeCategory::Planning, MistakeSeverity::Critical) => {
                "decompose goal into smaller objectives with explicit dependencies".into()
            }
            (MistakeCategory::Execution, MistakeSeverity::Critical) => {
                "verify tool capabilities before execution; add fallback tools".into()
            }
            (MistakeCategory::Reasoning, MistakeSeverity::Major) => {
                "gather more evidence before forming conclusions; use parallel hypothesis testing"
                    .into()
            }
            (MistakeCategory::Knowledge, MistakeSeverity::Minor) => {
                "update world model with new information; consult external sources".into()
            }
            (MistakeCategory::Resource, MistakeSeverity::Major) => {
                "adjust budget allocation; reduce parallelism when constrained".into()
            }
            (MistakeCategory::Safety, MistakeSeverity::Critical) => {
                "add explicit safety validation step before execution".into()
            }
            (MistakeCategory::Communication, MistakeSeverity::Major) => {
                "ask clarifying questions before proceeding; confirm understanding".into()
            }
            _ => "apply standard error recovery procedure".into(),
        }
    }

    /// Generate improvements from mistakes and root causes.
    fn generate_improvements(
        &self,
        mistakes: &[Mistake],
        root_causes: &[RootCause],
    ) -> Vec<Improvement> {
        let mut improvements = Vec::new();
        for (m, rc) in mistakes.iter().zip(root_causes.iter()) {
            let category = match m.category {
                MistakeCategory::Planning => ImprovementCategory::Strategy,
                MistakeCategory::Execution => ImprovementCategory::Coordination,
                MistakeCategory::Reasoning => ImprovementCategory::Prompt,
                MistakeCategory::Knowledge => ImprovementCategory::Knowledge,
                MistakeCategory::Resource => ImprovementCategory::Resource,
                MistakeCategory::Safety => ImprovementCategory::Safety,
                MistakeCategory::Communication => ImprovementCategory::Coordination,
                MistakeCategory::Timing => ImprovementCategory::Timing,
                MistakeCategory::Unknown => ImprovementCategory::Automation,
            };
            improvements.push(Improvement {
                id: format!("improve-{}", m.id),
                description: format!(
                    "address root cause: {} - fix: {}",
                    rc.primary, rc.fix_suggestion
                ),
                expected_impact: match m.severity {
                    MistakeSeverity::Critical => 0.8,
                    MistakeSeverity::Major => 0.5,
                    MistakeSeverity::Minor => 0.2,
                },
                category,
            });
        }
        improvements
    }

    /// Generate actionable, prioritized insights.
    fn generate_actionable_insights(
        &self,
        goal_id: GoalId,
        mistakes: &[Mistake],
        root_causes: &[RootCause],
    ) -> Vec<ActionableInsight> {
        let mut insights = Vec::new();
        for (i, (m, rc)) in mistakes.iter().zip(root_causes.iter()).enumerate() {
            let priority = match m.severity {
                MistakeSeverity::Critical => 1,
                MistakeSeverity::Major => 2,
                MistakeSeverity::Minor => 3,
            };
            let impact = match m.severity {
                MistakeSeverity::Critical => 0.9,
                MistakeSeverity::Major => 0.6,
                MistakeSeverity::Minor => 0.3,
            };
            let effort = match m.category {
                MistakeCategory::Planning => "medium",
                MistakeCategory::Execution => "short",
                MistakeCategory::Reasoning => "medium",
                MistakeCategory::Knowledge => "long",
                MistakeCategory::Resource => "short",
                MistakeCategory::Safety => "medium",
                MistakeCategory::Communication => "short",
                MistakeCategory::Timing => "short",
                MistakeCategory::Unknown => "long",
            };
            insights.push(ActionableInsight {
                id: format!("insight-{}-{}", goal_id, i),
                summary: rc.fix_suggestion.clone(),
                detail: format!(
                    "mistake '{}' caused by {}: {}. Fix: {}",
                    m.description,
                    rc.primary,
                    rc.contributing_factors.join(", "),
                    rc.fix_suggestion
                ),
                category: match m.category {
                    MistakeCategory::Planning => ImprovementCategory::Strategy,
                    MistakeCategory::Execution => ImprovementCategory::Coordination,
                    MistakeCategory::Reasoning => ImprovementCategory::Prompt,
                    MistakeCategory::Knowledge => ImprovementCategory::Knowledge,
                    MistakeCategory::Resource => ImprovementCategory::Resource,
                    MistakeCategory::Safety => ImprovementCategory::Safety,
                    MistakeCategory::Communication => ImprovementCategory::Coordination,
                    MistakeCategory::Timing => ImprovementCategory::Timing,
                    MistakeCategory::Unknown => ImprovementCategory::Automation,
                },
                priority,
                estimated_impact: impact,
                effort_estimate: effort.into(),
                applies_to_future_goals: vec![m.category.as_str().into()],
            });
        }
        insights
    }

    /// Extract lessons from mistakes, improvements, and insights.
    fn extract_lessons(
        &self,
        goal_id: GoalId,
        mistakes: &[Mistake],
        improvements: &[Improvement],
        insights: &[ActionableInsight],
    ) -> Vec<Lesson> {
        let count = mistakes.len().max(improvements.len()).max(insights.len());
        let mut lessons = Vec::with_capacity(count);
        for i in 0..count {
            let m = mistakes.get(i);
            let imp = improvements.get(i);
            let ins = insights.get(i);
            lessons.push(Lesson {
                lesson_id: LessonId::new(),
                goal_id,
                summary: ins.map(|i| i.summary.clone()).unwrap_or_else(|| {
                    m.map(|m| format!("lesson from {}: {}", m.category.as_str(), m.root_cause))
                        .unwrap_or_default()
                }),
                details: imp
                    .map(|i| format!("improvement: {}", i.description))
                    .unwrap_or_default(),
                category: m.map(|m| m.category).unwrap_or(MistakeCategory::Unknown),
                confidence: Confidence::new(
                    m.map(|m| match m.severity {
                        MistakeSeverity::Critical => 0.9,
                        MistakeSeverity::Major => 0.7,
                        MistakeSeverity::Minor => 0.5,
                    })
                    .unwrap_or(0.5),
                ),
                created_at: Timestamp::now(),
                applied_count: 0,
                tags: m
                    .map(|m| vec![m.category.as_str().into()])
                    .unwrap_or_default(),
            });
        }
        lessons
    }
}

impl Default for Reflector {
    fn default() -> Self {
        Self::new()
    }
}
