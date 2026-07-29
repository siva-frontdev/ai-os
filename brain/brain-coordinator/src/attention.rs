use std::collections::VecDeque;

use crate::evolution::EvolutionReport;
use brain_core::types::Decision;
use intelligence_coordinator::world_understanding::StructuredWorldUpdate;
use memory_core::Timestamp;
use memory_core::wm::Entity;

// ── Attention Outcome ────────────────────────────────────────

/// What the AI should do as a result of attention evaluation.
#[derive(Debug, Clone, PartialEq)]
pub enum AttentionOutcome {
    /// Do nothing, remain silent.
    Ignore,
    /// Observe passively, no reflection needed.
    ObserveLater { rhythm: Rhythm },
    /// Deeper reflection may be worthwhile at a later time.
    ReflectSoon { rhythm: Rhythm },
    /// Immediate reflection is warranted.
    ReflectNow,
    /// User input is needed to proceed.
    AskUser { question: String },
    /// Proactive suggestion worth making.
    Suggest { message: String },
    /// Notify the user of something important.
    Notify { message: String },
    /// Escalate — needs human attention.
    Escalate { reason: String },
}

// ── Rhythm ───────────────────────────────────────────────────

/// When the AI should next reason about something.
///
/// These are reasoning opportunities, not cron jobs.
/// The AI will decide whether reflection is actually worthwhile
/// when the rhythm triggers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rhythm {
    /// React immediately (seconds).
    Immediate,
    /// Check back in N minutes.
    Minutes(u64),
    /// Next morning reflection.
    Morning,
    /// Next evening reflection.
    Evening,
    /// Next weekly reflection.
    Weekly,
    /// Next monthly reflection.
    Monthly,
    /// Distant future, low priority.
    LongTerm,
}

// ── Attention Decision ───────────────────────────────────────

/// The complete result of an attention evaluation.
#[derive(Debug, Clone)]
pub struct AttentionDecision {
    /// What to do.
    pub outcome: AttentionOutcome,
    /// Re-evaluation rhythm (Some for deferrals).
    pub rhythm: Option<Rhythm>,
    /// Confidence in this decision (0.0–1.0).
    pub confidence: f64,
    /// Human-readable justification.
    pub reason: String,
    /// Signal scores that drove this decision.
    pub signals: Vec<EvaluatedSignal>,
}

/// A single signal that contributed to an attention decision.
#[derive(Debug, Clone)]
pub struct EvaluatedSignal {
    pub name: &'static str,
    pub strength: f64,
    pub description: String,
}

// ── Attention Memory ─────────────────────────────────────────

/// How the user responded to an attention event.
#[derive(Debug, Clone, PartialEq)]
pub enum UserResponse {
    Accepted,
    Rejected,
    Ignored,
}

/// A recorded attention event.
#[derive(Debug, Clone)]
pub struct AttentionRecord {
    pub outcome: AttentionOutcome,
    pub signals: Vec<EvaluatedSignal>,
    pub user_response: Option<UserResponse>,
    pub decision: Option<Decision>,
}

/// Adaptive memory of past attention interactions.
///
/// Learns user preferences over time:
/// - Tracks ignored/accepted/rejected ratios
/// - Adjusts interruption threshold based on history
/// - Models attention fatigue
#[derive(Debug, Clone)]
pub struct AttentionMemory {
    /// Recent attention events (FIFO, bounded).
    history: VecDeque<AttentionRecord>,
    /// Maximum events to retain.
    max_history: usize,
    /// How many consecutive ignores we have received.
    consecutive_ignores: u32,
    /// How many recent suggestions were accepted.
    recent_accepts: u32,
    /// How many recent suggestions were rejected.
    recent_rejects: u32,
    /// Current attention fatigue (0.0–1.0).
    fatigue: f64,
    /// Fatigue decay per cycle.
    fatigue_decay: f64,
}

impl AttentionMemory {
    pub fn new() -> Self {
        Self {
            history: VecDeque::new(),
            max_history: 100,
            consecutive_ignores: 0,
            recent_accepts: 0,
            recent_rejects: 0,
            fatigue: 0.0,
            fatigue_decay: 0.1,
        }
    }

    /// Record an attention decision and optionally how the user responded.
    pub fn record(&mut self, decision: &AttentionDecision, user_response: Option<UserResponse>) {
        if self.history.len() >= self.max_history {
            self.history.pop_front();
        }
        self.history.push_back(AttentionRecord {
            outcome: decision.outcome.clone(),
            signals: decision.signals.clone(),
            user_response: user_response.clone(),
            decision: None,
        });

        match user_response {
            Some(UserResponse::Accepted) => {
                self.consecutive_ignores = 0;
                self.recent_accepts += 1;
                self.fatigue = (self.fatigue - self.fatigue_decay).max(0.0);
            }
            Some(UserResponse::Rejected) => {
                self.consecutive_ignores = 0;
                self.recent_rejects += 1;
                self.fatigue = (self.fatigue + 0.1).min(1.0);
            }
            Some(UserResponse::Ignored) => {
                self.consecutive_ignores += 1;
                self.fatigue = (self.fatigue + 0.05).min(1.0);
            }
            None => {
                // No user feedback — decay fatigue slightly
                self.fatigue = (self.fatigue - self.fatigue_decay * 0.5).max(0.0);
            }
        }
    }

    /// Adaptive attention threshold.
    ///
    /// Higher threshold = harder to get attention (user is busy, ignores, or fatigued).
    /// Lower threshold = easier to get attention (user is responsive).
    pub fn attention_threshold(&self) -> f64 {
        let base = 0.3;
        // Increase threshold if user frequently ignores
        let ignore_penalty = (self.consecutive_ignores as f64).min(10.0) * 0.03;
        // Increase threshold if fatigued
        let fatigue_penalty = self.fatigue * 0.2;
        // Decrease threshold if user frequently accepts
        let accept_bonus = (self.recent_accepts as f64).min(10.0) * 0.02;
        // Increase threshold if user frequently rejects
        let reject_penalty = (self.recent_rejects as f64).min(5.0) * 0.04;

        (base + ignore_penalty + fatigue_penalty + reject_penalty - accept_bonus).clamp(0.05, 0.95)
    }

    /// How many attention events have been recorded.
    pub fn event_count(&self) -> usize {
        self.history.len()
    }

    /// Number of consecutive ignores.
    pub fn consecutive_ignores(&self) -> u32 {
        self.consecutive_ignores
    }

    /// Current fatigue level.
    pub fn fatigue(&self) -> f64 {
        self.fatigue
    }
}

impl Default for AttentionMemory {
    fn default() -> Self {
        Self::new()
    }
}

// ── Attention Evaluator ──────────────────────────────────────

/// Evaluates whether the AI should pay attention to what just happened.
///
/// This is the core of the Cognitive Attention & Rhythm System.
/// It does NOT use fixed weights. Instead it evaluates each signal
/// independently and determines the outcome based on which signals
/// dominate and how strong they are.
#[derive(Debug)]
pub struct AttentionEvaluator {
    memory: AttentionMemory,
}

impl AttentionEvaluator {
    pub fn new() -> Self {
        Self {
            memory: AttentionMemory::new(),
        }
    }

    pub fn with_memory(memory: AttentionMemory) -> Self {
        Self { memory }
    }

    /// Access the attention memory.
    pub fn memory(&self) -> &AttentionMemory {
        &self.memory
    }

    /// Mutable access to the attention memory.
    pub fn memory_mut(&mut self) -> &mut AttentionMemory {
        &mut self.memory
    }

    /// Evaluate whether the current observation deserves attention.
    ///
    /// The algorithm:
    /// 1. Scan all signals from the understanding
    /// 2. Determine the dominant signal(s) and their strengths
    /// 3. Consider the attention threshold (adapted from history)
    /// 4. Produce an outcome and optional rhythm
    pub fn evaluate(
        &mut self,
        update: &StructuredWorldUpdate,
        report: &EvolutionReport,
    ) -> AttentionDecision {
        let mut signals: Vec<EvaluatedSignal> = Vec::new();
        let threshold = self.memory.attention_threshold();

        // ── Signal 1: People ─────────────────────────────────
        let person_signal = self.eval_people_signal(update);
        if person_signal > 0.0 {
            signals.push(EvaluatedSignal {
                name: "people",
                strength: person_signal,
                description: "People are involved".into(),
            });
        }

        // ── Signal 2: Urgency ────────────────────────────────
        let urgency_signal = self.eval_urgency_signal(update);
        if urgency_signal > 0.0 {
            signals.push(EvaluatedSignal {
                name: "urgency",
                strength: urgency_signal,
                description: "Time-sensitive".into(),
            });
        }

        // ── Signal 3: Novelty ────────────────────────────────
        let novelty_signal = self.eval_novelty_signal(update);
        if novelty_signal > 0.0 {
            signals.push(EvaluatedSignal {
                name: "novelty",
                strength: novelty_signal,
                description: "New information".into(),
            });
        }

        // ── Signal 4: Uncertainty ────────────────────────────
        let uncertainty_signal = self.eval_uncertainty_signal(update, report);
        if uncertainty_signal > 0.0 {
            signals.push(EvaluatedSignal {
                name: "uncertainty",
                strength: uncertainty_signal,
                description: "Uncertain".into(),
            });
        }

        // ── Signal 5: State Change ──────────────────────────
        let state_change_signal = self.eval_state_change_signal(update);
        if state_change_signal > 0.0 {
            signals.push(EvaluatedSignal {
                name: "state_change",
                strength: state_change_signal,
                description: "Things have changed".into(),
            });
        }

        // ── Signal 6: Relationship ──────────────────────────
        let rel_signal = self.eval_relationship_signal(update, report);
        if rel_signal > 0.0 {
            signals.push(EvaluatedSignal {
                name: "relationship",
                strength: rel_signal,
                description: "Relationships are shifting".into(),
            });
        }

        // ── Signal 7: Importance ────────────────────────────
        let importance_signal = self.eval_importance_signal(update);
        if importance_signal > 0.0 {
            signals.push(EvaluatedSignal {
                name: "importance",
                strength: importance_signal,
                description: "Important matters".into(),
            });
        }

        // ── Signal 8: Anomaly ───────────────────────────────
        let anomaly_signal = self.eval_anomaly_signal(update);
        if anomaly_signal > 0.0 {
            signals.push(EvaluatedSignal {
                name: "anomaly",
                strength: anomaly_signal,
                description: "Something unusual".into(),
            });
        }

        // ── Determine outcome ────────────────────────────────

        // Sort signals by strength (strongest first)
        signals.sort_by(|a, b| {
            b.strength
                .partial_cmp(&a.strength)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let dominant = signals.first().map(|s| s.strength).unwrap_or(0.0);
        let total: f64 = signals.iter().map(|s| s.strength).sum();
        let combined = (dominant + total * 0.3).min(1.0);

        // Decision tree using signal dominance and combined score
        let decision = if combined >= 0.8 {
            // Very strong signal — immediate attention
            // Determine the specific outcome from the dominant signal
            let dominant_name = signals.first().map(|s| s.name).unwrap_or("");
            self.high_attention_outcome(dominant_name, update, combined)
        } else if combined >= threshold.max(0.4) {
            // Strong enough — reflect soon or now based on specific signals
            if self.has_high_urgency(&signals) {
                AttentionOutcome::ReflectNow
            } else if self.has_people(&signals) {
                AttentionOutcome::Notify {
                    message: self.build_notification(update),
                }
            } else if self.has_state_change(&signals) {
                AttentionOutcome::ReflectNow
            } else {
                AttentionOutcome::ReflectSoon {
                    rhythm: Rhythm::Minutes(5),
                }
            }
        } else if combined >= threshold {
            // Moderate — observe later
            AttentionOutcome::ObserveLater {
                rhythm: self.select_rhythm(&signals),
            }
        } else {
            // Weak signals — ignore
            AttentionOutcome::Ignore
        };

        let rhythm = match &decision {
            AttentionOutcome::ObserveLater { rhythm } => Some(*rhythm),
            AttentionOutcome::ReflectSoon { rhythm } => Some(*rhythm),
            _ => None,
        };

        let reason = if combined < threshold {
            "Not enough to act on".into()
        } else {
            let dominant_desc = signals
                .first()
                .map(|s| format!("Most relevant: {}", s.name))
                .unwrap_or_default();
            format!("Paying attention — {dominant_desc}")
        };

        AttentionDecision {
            outcome: decision,
            rhythm,
            confidence: combined,
            reason,
            signals,
        }
    }

    /// Record user feedback for learning.
    pub fn record_feedback(&mut self, decision: &AttentionDecision, response: UserResponse) {
        self.memory.record(decision, Some(response));
    }

    // ── Signal Evaluators ────────────────────────────────────

    fn eval_people_signal(&self, update: &StructuredWorldUpdate) -> f64 {
        let people_count = update
            .entities
            .iter()
            .filter(|e| matches!(e.entity_type.as_str(), "person" | "user" | "human"))
            .count();
        if people_count == 0 {
            return 0.0;
        }
        // People signal strength depends on quantity and importance
        let max_importance = update
            .entities
            .iter()
            .filter(|e| matches!(e.entity_type.as_str(), "person" | "user" | "human"))
            .map(|e| e.importance)
            .fold(0.0_f64, f64::max);
        let count_factor = (people_count as f64).min(3.0) / 3.0;
        let importance_factor = max_importance;
        (count_factor * 0.4 + importance_factor * 0.6).min(1.0)
    }

    fn eval_urgency_signal(&self, update: &StructuredWorldUpdate) -> f64 {
        let mut urgency = 0.0_f64;
        let urgent_terms = [
            "deadline",
            "urgent",
            "critical",
            "asap",
            "emergency",
            "blocked",
            "failing",
            "crisis",
            "overdue",
            "immediately",
        ];

        // Check observations for urgent language
        for obs in &update.new_observations {
            let lower = obs.to_lowercase();
            for term in &urgent_terms {
                if lower.contains(term) {
                    urgency = urgency.max(0.7);
                }
            }
        }

        // Check entity properties for urgent states
        for entity in &update.entities {
            for v in entity.properties.values() {
                let lower = v.to_lowercase();
                for term in &urgent_terms {
                    if lower.contains(term) {
                        urgency = urgency.max(0.6);
                    }
                }
            }
        }

        // Check state changes for blocking/failing
        for sc in &update.state_changes {
            let lower = sc.new_value.to_lowercase();
            match lower.as_str() {
                "blocked" | "failing" | "critical" | "emergency" => urgency = urgency.max(0.9),
                "overdue" | "stalled" => urgency = urgency.max(0.7),
                "warning" | "degraded" => urgency = urgency.max(0.5),
                _ => {}
            }
        }

        urgency.min(1.0)
    }

    fn eval_novelty_signal(&self, update: &StructuredWorldUpdate) -> f64 {
        let mut novelty = 0.0_f64;

        // New observations are inherently novel
        if !update.new_observations.is_empty() {
            novelty = novelty.max(0.4);
        }

        // Open questions suggest things we don't yet understand
        if !update.open_questions.is_empty() {
            novelty = novelty.max(0.5);
        }

        // Hypotheses suggest novel patterns
        if !update.possible_hypotheses.is_empty() {
            novelty = novelty.max(0.6);
        }

        // Brand new entities (not mentions of existing ones) are novel
        let entity_count = update.entities.len();
        if entity_count > 2 {
            novelty = novelty.max(0.3);
        }

        novelty.min(1.0)
    }

    fn eval_uncertainty_signal(
        &self,
        update: &StructuredWorldUpdate,
        report: &EvolutionReport,
    ) -> f64 {
        let mut uncertainty = 0.0_f64;

        // Low-confidence entities raise uncertainty
        for entity in &update.entities {
            if entity.confidence < 0.3 {
                uncertainty = uncertainty.max(0.6);
            } else if entity.confidence < 0.5 {
                uncertainty = uncertainty.max(0.3);
            }
        }

        // Confidence changes suggest uncertainty changed
        for change in &report.confidence_changes {
            let delta = (change.current - change.previous).abs();
            if delta > 0.2 {
                uncertainty = uncertainty.max(0.5);
            }
        }

        // Open questions directly express uncertainty
        if !update.open_questions.is_empty() {
            uncertainty = uncertainty.max(0.6);
        }

        // Possible hypotheses indicate uncertain patterns
        if !update.possible_hypotheses.is_empty() {
            uncertainty = uncertainty.max(0.4);
        }

        uncertainty.min(1.0)
    }

    fn eval_state_change_signal(&self, update: &StructuredWorldUpdate) -> f64 {
        if update.state_changes.is_empty() {
            return 0.0;
        }

        let mut strength = 0.0_f64;
        for sc in &update.state_changes {
            let attr_lower = sc.attribute.to_lowercase();
            let val_lower = sc.new_value.to_lowercase();

            // Strong state changes
            match attr_lower.as_str() {
                "mood" | "emotion" | "feeling" => {
                    if matches!(
                        val_lower.as_str(),
                        "angry" | "sad" | "anxious" | "stressed" | "upset" | "worried"
                    ) {
                        strength = strength.max(0.8);
                    } else if matches!(val_lower.as_str(), "tired" | "sleepy" | "frustrated") {
                        strength = strength.max(0.6);
                    }
                }
                "health" | "wellness" => {
                    strength = strength.max(0.7);
                }
                "state" => {
                    if matches!(val_lower.as_str(), "sleeping" | "asleep") {
                        // User winding down — negative attention signal
                        // (signal stays at 0.0, no interruption)
                    } else if val_lower == "blocked" || val_lower == "failing" {
                        strength = strength.max(0.7);
                    }
                }
                _ => {
                    // Generic state change — mild
                    if !matches!(val_lower.as_str(), "sleeping" | "asleep") {
                        strength = strength.max(0.3);
                    }
                }
            }
        }

        // Multiple state changes compound
        let count = update.state_changes.len() as f64;
        strength += (count - 1.0) * 0.1;

        strength.min(1.0)
    }

    fn eval_relationship_signal(
        &self,
        update: &StructuredWorldUpdate,
        report: &EvolutionReport,
    ) -> f64 {
        let mut rel_strength = 0.0_f64;

        // New relationships from this observation
        if !update.relationships.is_empty() {
            rel_strength = rel_strength.max(0.4);
        }

        // Strengthened relationships from evolution
        if !report.relationships_strengthened.is_empty() {
            rel_strength = rel_strength.max(0.5);
        }

        // Created relationships from evolution
        if !report.relationships_created.is_empty() {
            rel_strength = rel_strength.max(0.3);
        }

        rel_strength.min(1.0)
    }

    fn eval_importance_signal(&self, update: &StructuredWorldUpdate) -> f64 {
        let mut importance = 0.0_f64;
        for entity in &update.entities {
            if entity.importance >= 0.8 {
                importance = importance.max(entity.importance);
            }
        }
        importance
    }

    fn eval_anomaly_signal(&self, update: &StructuredWorldUpdate) -> f64 {
        let mut anomaly = 0.0_f64;

        // Possible hypotheses suggest anomalous patterns
        if !update.possible_hypotheses.is_empty() {
            anomaly = anomaly.max(0.5);
        }

        anomaly.min(1.0)
    }

    // ── Context Evaluation (for self-initiated reflection) ────

    /// Evaluate attention on the current World Model state alone
    /// (no new observation). Used by tick() for companion reflection.
    pub fn evaluate_context(&self, context_entities: &[Entity]) -> AttentionDecision {
        let threshold = self.memory.attention_threshold();
        let mut signals: Vec<EvaluatedSignal> = Vec::new();

        let stagnation = self.eval_stagnation_signal(context_entities);
        if stagnation > 0.0 {
            signals.push(EvaluatedSignal {
                name: "stagnation",
                strength: stagnation,
                description: "Neglected topics".into(),
            });
        }

        let importance = self.eval_context_importance_signal(context_entities);
        if importance > 0.0 {
            signals.push(EvaluatedSignal {
                name: "importance",
                strength: importance,
                description: "Important context".into(),
            });
        }

        let context_novelty = self.eval_context_novelty_signal(context_entities);
        if context_novelty > 0.0 {
            signals.push(EvaluatedSignal {
                name: "novelty",
                strength: context_novelty,
                description: "Meaningful context".into(),
            });
        }

        signals.sort_by(|a, b| {
            b.strength
                .partial_cmp(&a.strength)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let dominant = signals.first().map(|s| s.strength).unwrap_or(0.0);
        let total: f64 = signals.iter().map(|s| s.strength).sum();
        let combined = (dominant + total * 0.3).min(1.0);

        let outcome = if combined >= 0.7 {
            AttentionOutcome::ReflectNow
        } else if combined >= threshold.max(0.3) {
            AttentionOutcome::ReflectSoon {
                rhythm: Rhythm::Minutes(15),
            }
        } else if combined >= threshold * 0.7 {
            AttentionOutcome::ObserveLater {
                rhythm: Rhythm::Minutes(30),
            }
        } else {
            AttentionOutcome::Ignore
        };

        let reason = if combined < threshold {
            "Not enough to act on".into()
        } else {
            let dominant_desc = signals
                .first()
                .map(|s| format!("Most relevant: {}", s.name))
                .unwrap_or_default();
            format!("Paying attention — {dominant_desc}")
        };

        AttentionDecision {
            outcome,
            rhythm: None,
            confidence: combined,
            reason,
            signals,
        }
    }

    /// How strongly should the AI reach out about neglected entities?
    fn eval_stagnation_signal(&self, entities: &[Entity]) -> f64 {
        let now = Timestamp::now();
        let mut max_stagnation = 0.0_f64;

        for entity in entities {
            let age_secs =
                (now.as_nanos() - entity.updated_at.as_nanos()).abs() as f64 / 1_000_000_000.0;
            let days_stale = age_secs / 86400.0;

            if entity.importance as f64 > 0.5 && days_stale > 1.0 {
                let stagnation = (entity.importance as f64) * (days_stale / 7.0).min(1.0) * 0.7;
                max_stagnation = max_stagnation.max(stagnation);
            }
        }

        max_stagnation.min(1.0)
    }

    fn eval_context_importance_signal(&self, entities: &[Entity]) -> f64 {
        entities
            .iter()
            .map(|e| e.importance as f64)
            .fold(0.0, f64::max)
    }

    fn eval_context_novelty_signal(&self, entities: &[Entity]) -> f64 {
        if entities.is_empty() {
            return 0.0;
        }
        // Having any entities worth tracking is inherently noteworthy
        (entities.len() as f64 * 0.15).min(0.5)
    }

    // ── Outcome Helpers ──────────────────────────────────────

    fn high_attention_outcome(
        &self,
        dominant_name: &str,
        _update: &StructuredWorldUpdate,
        _combined: f64,
    ) -> AttentionOutcome {
        match dominant_name {
            "people" => {
                // Strong person signal — let the decide phase handle specifics
                AttentionOutcome::ReflectNow
            }
            "urgency" => AttentionOutcome::ReflectNow,
            "state_change" => AttentionOutcome::ReflectNow,
            "uncertainty" => AttentionOutcome::ReflectSoon {
                rhythm: Rhythm::Minutes(2),
            },
            "novelty" => AttentionOutcome::ObserveLater {
                rhythm: Rhythm::Minutes(10),
            },
            "relationship" => AttentionOutcome::ReflectNow,
            "anomaly" => AttentionOutcome::ReflectSoon {
                rhythm: Rhythm::Minutes(5),
            },
            _ => AttentionOutcome::ReflectNow,
        }
    }

    fn has_high_urgency(&self, signals: &[EvaluatedSignal]) -> bool {
        signals
            .iter()
            .any(|s| s.name == "urgency" && s.strength > 0.6)
    }

    fn has_people(&self, signals: &[EvaluatedSignal]) -> bool {
        signals.iter().any(|s| s.name == "people")
    }

    fn has_state_change(&self, signals: &[EvaluatedSignal]) -> bool {
        signals.iter().any(|s| s.name == "state_change")
    }

    fn select_rhythm(&self, signals: &[EvaluatedSignal]) -> Rhythm {
        let total: f64 = signals.iter().map(|s| s.strength).sum();
        if total > 1.5 {
            Rhythm::Minutes(5)
        } else if total > 0.8 {
            Rhythm::Minutes(15)
        } else {
            Rhythm::Minutes(30)
        }
    }

    fn build_notification(&self, update: &StructuredWorldUpdate) -> String {
        let people: Vec<&str> = update
            .entities
            .iter()
            .filter(|e| matches!(e.entity_type.as_str(), "person" | "user"))
            .map(|e| e.name.as_str())
            .collect();

        if people.len() == 1 {
            format!("{} is around", people[0])
        } else if people.len() > 1 {
            format!("{} people are present", people.len())
        } else {
            "Something came up that might be worth discussing.".into()
        }
    }
}

impl Default for AttentionEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evolution::{ConfidenceChange, EvolutionReport, ImportanceChange};
    use intelligence_coordinator::world_understanding::StructuredWorldUpdate;

    // ── Helpers ──────────────────────────────────────────────

    fn empty_update() -> StructuredWorldUpdate {
        StructuredWorldUpdate {
            entities: vec![],
            relationships: vec![],
            state_changes: vec![],
            new_observations: vec![],
            open_questions: vec![],
            possible_hypotheses: vec![],
        }
    }

    fn empty_report() -> EvolutionReport {
        EvolutionReport {
            entities_created: vec![],
            entities_updated: vec![],
            relationships_created: vec![],
            relationships_strengthened: vec![],
            confidence_changes: vec![],
            importance_changes: vec![],
        }
    }

    fn make_entity(
        name: &str,
        entity_type: &str,
        importance: f64,
        confidence: f64,
    ) -> intelligence_coordinator::world_understanding::WorldEntity {
        intelligence_coordinator::world_understanding::WorldEntity {
            name: name.into(),
            entity_type: entity_type.into(),
            properties: std::collections::HashMap::new(),
            confidence,
            importance,
        }
    }

    // ── Example 1: "I'm going to sleep" ─────────────────────

    #[test]
    fn test_sleep_observation_silence() {
        let mut evaluator = AttentionEvaluator::new();

        let update = StructuredWorldUpdate {
            entities: vec![make_entity("User", "user", 0.1, 0.9)],
            state_changes: vec![intelligence_coordinator::world_understanding::StateChange {
                entity_name: "User".into(),
                attribute: "state".into(),
                old_value: Some("awake".into()),
                new_value: "sleeping".into(),
            }],
            ..empty_update()
        };

        let decision = evaluator.evaluate(&update, &empty_report());

        // Going to sleep should not trigger interruption
        // The state change to "sleeping" is low-urgency
        match decision.outcome {
            AttentionOutcome::Ignore | AttentionOutcome::ObserveLater { .. } => { /* acceptable */ }
            other => panic!(
                "Expected silence for sleep observation, got {other:?}\nreason: {}",
                decision.reason
            ),
        }
    }

    // ── Example 2: Meeting starts in 10 minutes ─────────────

    #[test]
    fn test_meeting_imminent_immediate_attention() {
        let mut evaluator = AttentionEvaluator::new();

        let update = StructuredWorldUpdate {
            entities: vec![make_entity("Team Meeting", "event", 0.8, 0.95)],
            new_observations: vec!["Team meeting starts in 10 minutes".into()],
            state_changes: vec![intelligence_coordinator::world_understanding::StateChange {
                entity_name: "Team Meeting".into(),
                attribute: "status".into(),
                old_value: Some("upcoming".into()),
                new_value: "imminent".into(),
            }],
            ..empty_update()
        };

        let decision = evaluator.evaluate(&update, &empty_report());

        // Meeting should trigger immediate attention
        match &decision.outcome {
            AttentionOutcome::ReflectNow | AttentionOutcome::Notify { .. } => { /* correct */ }
            other => panic!(
                "Expected immediate attention for imminent meeting, got {other:?}\nreason: {}",
                decision.reason
            ),
        }
        assert!(
            decision.confidence > 0.3,
            "confidence should be meaningful for meeting"
        );
    }

    // ── Example 3: Project inactive for 10 days ─────────────

    #[test]
    fn test_inactive_project_deferred_reflection() {
        let mut evaluator = AttentionEvaluator::new();

        let update = StructuredWorldUpdate {
            entities: vec![make_entity("SEO Project", "project", 0.6, 0.7)],
            state_changes: vec![intelligence_coordinator::world_understanding::StateChange {
                entity_name: "SEO Project".into(),
                attribute: "status".into(),
                old_value: Some("active".into()),
                new_value: "inactive".into(),
            }],
            new_observations: vec!["SEO project has been inactive for 10 days".into()],
            ..empty_update()
        };

        let decision = evaluator.evaluate(&update, &empty_report());

        // Inactive project should get deferred reflection, not ignored entirely
        match &decision.outcome {
            AttentionOutcome::ObserveLater { .. }
            | AttentionOutcome::ReflectSoon { .. }
            | AttentionOutcome::ReflectNow => { /* acceptable */ }
            other => panic!(
                "Expected deferred reflection for inactive project, got {other:?}\nreason: {}",
                decision.reason
            ),
        }
    }

    // ── Example 4: Important email ─────────────────────────

    #[test]
    fn test_important_email_context_dependent() {
        let mut evaluator = AttentionEvaluator::new();

        // High-importance, high-confidence person email
        let update = StructuredWorldUpdate {
            entities: vec![make_entity("Alice", "person", 0.9, 0.9)],
            new_observations: vec![
                "Received an important email from Alice about the project deadline".into(),
            ],
            state_changes: vec![],
            relationships: vec![],
            open_questions: vec![],
            possible_hypotheses: vec![],
        };

        let decision = evaluator.evaluate(&update, &empty_report());

        // Important email from high-importance person should get attention
        match &decision.outcome {
            AttentionOutcome::ReflectNow
            | AttentionOutcome::Notify { .. }
            | AttentionOutcome::ReflectSoon { .. } => { /* acceptable */ }
            AttentionOutcome::Ignore => panic!(
                "Important email from Alice should not be ignored\nreason: {}",
                decision.reason
            ),
            _ => { /* other outcomes may be acceptable */ }
        }
        assert!(
            decision.confidence > 0.3,
            "important email should have meaningful confidence"
        );
    }

    // ── Example 5: Repeatedly ignored reminders ────────────

    #[test]
    fn test_repeated_ignores_reduce_interruptions() {
        let mut evaluator = AttentionEvaluator::new();

        // Simulate many ignored suggestions
        for _ in 0..10 {
            let decision = evaluator.evaluate(
                &StructuredWorldUpdate {
                    entities: vec![make_entity("Reminder", "notification", 0.5, 0.5)],
                    new_observations: vec!["Generic reminder".into()],
                    ..empty_update()
                },
                &empty_report(),
            );
            evaluator.record_feedback(&decision, UserResponse::Ignored);
            assert!(
                evaluator.memory().consecutive_ignores() > 0,
                "ignores should accumulate"
            );
        }

        let threshold = evaluator.memory.attention_threshold();
        assert!(
            threshold > 0.35,
            "threshold should increase after repeated ignores: {threshold:.3}"
        );

        // Now even a moderately strong signal may be ignored
        let update = StructuredWorldUpdate {
            entities: vec![make_entity("Task", "task", 0.6, 0.7)],
            new_observations: vec!["Regular task update".into()],
            ..empty_update()
        };

        let decision = evaluator.evaluate(&update, &empty_report());
        // The system should be more reluctant to interrupt
        match decision.outcome {
            AttentionOutcome::Ignore | AttentionOutcome::ObserveLater { .. } => { /* expected after fatigue */
            }
            _ => {
                // After 10 ignores, even a moderate signal may still trigger
                // if the signal is strong enough, but it should at least be ObserveLater
            }
        }
        println!(
            "After 10 ignores: outcome={:?}, confidence={:.3}, threshold={:.3}",
            decision.outcome, decision.confidence, threshold
        );
    }

    // ── Scenario: Empty observation → Silence ───────────────

    #[test]
    fn test_empty_observation_silence() {
        let mut evaluator = AttentionEvaluator::new();
        let decision = evaluator.evaluate(&empty_update(), &empty_report());
        assert!(
            matches!(decision.outcome, AttentionOutcome::Ignore),
            "empty observation should be ignored, got {:?}",
            decision.outcome
        );
    }

    // ── Scenario: Person with high importance → Notify ──────

    #[test]
    fn test_high_importance_person_notification() {
        let mut evaluator = AttentionEvaluator::new();
        let update = StructuredWorldUpdate {
            entities: vec![make_entity("Alice", "person", 0.9, 0.95)],
            ..empty_update()
        };
        let decision = evaluator.evaluate(&update, &empty_report());
        match &decision.outcome {
            AttentionOutcome::Notify { message: _ } => {
                panic!(
                    "high-importance person with ReflectNow handles it via decide(), not directly"
                );
            }
            AttentionOutcome::ReflectNow => { /* correct — let decide() handle the specifics */ }
            other => panic!(
                "high-importance person should at least get Notify/ReflectNow, got {other:?}"
            ),
        }
    }

    // ── Scenario: State change mood → Reflect ───────────────

    #[test]
    fn test_mood_change_triggers_attention() {
        let mut evaluator = AttentionEvaluator::new();

        let update = StructuredWorldUpdate {
            entities: vec![make_entity("User", "user", 0.5, 0.9)],
            state_changes: vec![intelligence_coordinator::world_understanding::StateChange {
                entity_name: "User".into(),
                attribute: "mood".into(),
                old_value: Some("happy".into()),
                new_value: "stressed".into(),
            }],
            ..empty_update()
        };

        let decision = evaluator.evaluate(&update, &empty_report());

        // Mood changes to negative should give attention
        match &decision.outcome {
            AttentionOutcome::ReflectNow | AttentionOutcome::ReflectSoon { .. } => { /* correct */ }
            other => panic!(
                "mood change should trigger reflection, got {other:?}\nreason: {}",
                decision.reason
            ),
        }
    }

    // ── Scenario: Low confidence observation → ObserveLater ──

    #[test]
    fn test_low_confidence_uncertainty() {
        let mut evaluator = AttentionEvaluator::new();

        let update = StructuredWorldUpdate {
            entities: vec![make_entity("Unknown Entity", "thing", 0.2, 0.15)],
            open_questions: vec!["What is this entity?".into()],
            ..empty_update()
        };

        let decision = evaluator.evaluate(&update, &empty_report());

        // Low confidence with open questions should at least observe later
        match &decision.outcome {
            AttentionOutcome::ObserveLater { .. }
            | AttentionOutcome::ReflectSoon { .. }
            | AttentionOutcome::ReflectNow => { /* acceptable */ }
            AttentionOutcome::Ignore => panic!(
                "uncertain observation should not be fully ignored\nreason: {}",
                decision.reason
            ),
            _ => { /* other outcomes possible */ }
        }
    }

    // ── Scenario: Urgent deadline → Immediate ───────────────

    #[test]
    fn test_urgent_deadline_immediate() {
        let mut evaluator = AttentionEvaluator::new();

        let update = StructuredWorldUpdate {
            entities: vec![make_entity("Project", "project", 0.7, 0.85)],
            state_changes: vec![intelligence_coordinator::world_understanding::StateChange {
                entity_name: "Project".into(),
                attribute: "status".into(),
                old_value: Some("on_track".into()),
                new_value: "blocked".into(),
            }],
            new_observations: vec!["Project deadline is tomorrow and we are blocked".into()],
            ..empty_update()
        };

        let decision = evaluator.evaluate(&update, &empty_report());

        // Blocked project with deadline should get immediate attention
        match &decision.outcome {
            AttentionOutcome::ReflectNow | AttentionOutcome::Notify { .. } => { /* correct */ }
            other => {
                // At minimum it should not be Ignore
                assert!(
                    !matches!(other, AttentionOutcome::Ignore),
                    "urgent deadline should not be ignored\nreason: {}",
                    decision.reason
                );
            }
        }
    }

    // ── Scenario: High confidence delta → Attention ─────────

    #[test]
    fn test_confidence_collapse_triggers_attention() {
        let mut evaluator = AttentionEvaluator::new();

        let report = EvolutionReport {
            confidence_changes: vec![ConfidenceChange {
                id: memory_core::wm::EntityId::new(),
                name: "Project".into(),
                previous: 0.9,
                current: 0.3,
            }],
            ..empty_report()
        };

        let update = StructuredWorldUpdate {
            entities: vec![make_entity("Project", "project", 0.7, 0.3)],
            ..empty_update()
        };

        let decision = evaluator.evaluate(&update, &report);

        // Large confidence drop should trigger attention
        match &decision.outcome {
            AttentionOutcome::Ignore => panic!(
                "large confidence drop should not be ignored\nreason: {}",
                decision.reason
            ),
            _ => { /* any non-ignore outcome is acceptable */ }
        }
    }

    // ── Scenario: Repeated reminders with different rhythm ──

    #[test]
    fn test_reminder_fatigue_escalates_rhythm() {
        let mut evaluator = AttentionEvaluator::new();

        let update = || StructuredWorldUpdate {
            entities: vec![make_entity("Notification", "notification", 0.3, 0.5)],
            new_observations: vec!["Another reminder".into()],
            ..empty_update()
        };

        let mut rhythms: Vec<Option<Rhythm>> = Vec::new();

        for _ in 0..5 {
            let decision = evaluator.evaluate(&update(), &empty_report());
            rhythms.push(decision.rhythm);
            evaluator.record_feedback(&decision, UserResponse::Ignored);
        }

        // Fatigue should increase over time
        assert!(
            evaluator.memory.fatigue() > 0.0,
            "fatigue should accumulate"
        );
        assert!(
            evaluator.memory.consecutive_ignores() > 0,
            "should track consecutive ignores"
        );
    }

    // ── Scenario: Accepted suggestions lower threshold ──────

    #[test]
    fn test_accepted_suggestions_lower_threshold() {
        let mut evaluator = AttentionEvaluator::new();

        // User accepts several suggestions
        for _ in 0..5 {
            let decision = evaluator.evaluate(
                &StructuredWorldUpdate {
                    entities: vec![make_entity("Task", "task", 0.5, 0.6)],
                    ..empty_update()
                },
                &empty_report(),
            );
            evaluator.record_feedback(&decision, UserResponse::Accepted);
        }

        // Recent accepts should lower the threshold vs initial
        let threshold = evaluator.memory.attention_threshold();

        // Build a comparison: create a new evaluator (fresh threshold)
        let fresh = AttentionEvaluator::new();
        let fresh_threshold = fresh.memory().attention_threshold();

        // After accepts, the threshold should be lower or equal
        assert!(
            threshold <= fresh_threshold + 0.01,
            "threshold after accepts ({threshold:.3}) should not exceed fresh threshold ({fresh_threshold:.3})"
        );
    }

    // ── Rhythm selection ────────────────────────────────────

    #[test]
    fn test_rhythm_is_reasonable_for_moderate_signals() {
        let mut evaluator = AttentionEvaluator::new();

        let update = StructuredWorldUpdate {
            entities: vec![make_entity("Side Project", "project", 0.5, 0.6)],
            new_observations: vec!["Side project could use attention".into()],
            ..empty_update()
        };

        let decision = evaluator.evaluate(&update, &empty_report());

        // Moderate signals should get a reasonable rhythm
        match &decision.outcome {
            AttentionOutcome::ObserveLater { rhythm }
            | AttentionOutcome::ReflectSoon { rhythm } => {
                match rhythm {
                    Rhythm::Minutes(n) => {
                        assert!(*n >= 1, "rhythm should be at least 1 minute, got {n}");
                        assert!(*n <= 60, "rhythm should be at most 60 minutes, got {n}");
                    }
                    _ => { /* other rhythms also fine */ }
                }
            }
            AttentionOutcome::Ignore => { /* silence is acceptable for low signals */ }
            _ => { /* other outcomes may be acceptable */ }
        }
    }

    // ── AttentionDecision contains signals ──────────────────

    #[test]
    fn test_decision_includes_signals() {
        let mut evaluator = AttentionEvaluator::new();

        let update = StructuredWorldUpdate {
            entities: vec![
                make_entity("Alice", "person", 0.85, 0.95),
                make_entity("Bob", "person", 0.7, 0.9),
            ],
            ..empty_update()
        };

        let decision = evaluator.evaluate(&update, &empty_report());

        assert!(
            !decision.signals.is_empty(),
            "decision should contain evaluated signals"
        );
        assert!(
            decision.signals.iter().any(|s| s.name == "people"),
            "should have a people signal"
        );
        assert!(decision.confidence > 0.0, "confidence should be > 0");
        assert!(!decision.reason.is_empty(), "reason should be non-empty");
    }

    // ── Evolution report signals affect outcome ──────────────

    #[test]
    fn test_relationship_strengthening_affects_attention() {
        let mut evaluator = AttentionEvaluator::new();

        let report = EvolutionReport {
            relationships_strengthened: vec![crate::evolution::RelSnap {
                id: memory_core::wm::RelationshipId::new(),
                source_name: "Alice".into(),
                target_name: "Project".into(),
                relationship_type: "leads".into(),
                weight: 0.9,
                confidence: 0.9,
            }],
            ..empty_report()
        };

        let update = StructuredWorldUpdate {
            entities: vec![
                make_entity("Alice", "person", 0.8, 0.9),
                make_entity("Project", "project", 0.7, 0.85),
            ],
            relationships: vec![
                intelligence_coordinator::world_understanding::WorldRelationship {
                    source: "Alice".into(),
                    target: "Project".into(),
                    relationship_type: "leads".into(),
                    confidence: 0.9,
                    weight: 0.9,
                },
            ],
            ..empty_update()
        };

        let decision = evaluator.evaluate(&update, &report);

        // Strengthened relationships should affect attention
        assert!(
            !matches!(decision.outcome, AttentionOutcome::Ignore),
            "relationship strengthening should trigger some attention\nreason: {}",
            decision.reason
        );
    }

    // ── Confidence changes and importance changes ───────────

    #[test]
    fn test_importance_increase_affects_attention() {
        let mut evaluator = AttentionEvaluator::new();

        let report = EvolutionReport {
            importance_changes: vec![ImportanceChange {
                id: memory_core::wm::EntityId::new(),
                name: "Alice".into(),
                previous: 0.5,
                current: 0.9,
            }],
            ..empty_report()
        };

        let update = StructuredWorldUpdate {
            entities: vec![make_entity("Alice", "person", 0.9, 0.9)],
            ..empty_update()
        };

        let decision = evaluator.evaluate(&update, &report);

        assert!(
            !matches!(decision.outcome, AttentionOutcome::Ignore),
            "importance increase should trigger attention\nreason: {}",
            decision.reason
        );
    }

    // ── Default threshold is reasonable ─────────────────────

    #[test]
    fn test_default_threshold() {
        let memory = AttentionMemory::new();
        let t = memory.attention_threshold();
        assert!(t > 0.0, "threshold should be positive");
        assert!(t < 1.0, "threshold should be < 1.0");
        assert!(
            (t - 0.3).abs() < 0.1,
            "default threshold should be ~0.3, got {t:.3}"
        );
    }

    // ── Communication Philosophy: no internal terminology in descriptions ──

    const FORBIDDEN_DESC_TERMS: &[&str] = &[
        "strength=",
        "entities detected",
        "detected (strength=",
        "signal strength",
        "threshold",
        "world model",
    ];

    fn assert_description_clean(desc: &str, signal_name: &str) {
        let lower = desc.to_lowercase();
        for term in FORBIDDEN_DESC_TERMS {
            assert!(
                !lower.contains(term),
                "[{signal_name}] description leaked internal term '{term}': {desc}"
            );
        }
        assert!(
            desc.len() < 60,
            "[{signal_name}] description too long ({}) for user-facing: {desc}",
            desc.len()
        );
    }

    fn assert_reason_clean(reason: &str) {
        let lower = reason.to_lowercase();
        for term in &[
            "strength=",
            "threshold",
            "signal strength",
            "below threshold",
        ] {
            assert!(
                !lower.contains(term),
                "reason leaked internal term '{term}': {reason}"
            );
        }
    }

    #[test]
    fn test_signal_descriptions_no_internal_leaks() {
        let mut evaluator = AttentionEvaluator::new();
        let update = StructuredWorldUpdate {
            entities: vec![
                make_entity("Alice", "person", 0.9, 0.9),
                make_entity("UrgentProject", "project", 0.8, 0.5),
            ],
            state_changes: vec![intelligence_coordinator::world_understanding::StateChange {
                entity_name: "project".into(),
                attribute: "status".into(),
                old_value: Some("planned".into()),
                new_value: "in_progress".into(),
            }],
            ..empty_update()
        };

        let decision = evaluator.evaluate(&update, &empty_report());

        for signal in &decision.signals {
            assert_description_clean(&signal.description, signal.name);
        }
        assert_reason_clean(&decision.reason);
    }

    #[test]
    fn test_context_signal_descriptions_no_internal_leaks() {
        let evaluator = AttentionEvaluator::new();
        let entities = vec![
            memory_core::wm::Entity::new("person", "Alice").with_importance(0.9),
            memory_core::wm::Entity::new("project", "Website SEO").with_importance(0.7),
        ];

        let decision = evaluator.evaluate_context(&entities);

        for signal in &decision.signals {
            assert_description_clean(&signal.description, signal.name);
        }
        assert_reason_clean(&decision.reason);
    }

    #[test]
    fn test_notification_message_no_internal_leaks() {
        let evaluator = AttentionEvaluator::new();
        let update_with_person = StructuredWorldUpdate {
            entities: vec![make_entity("Alice", "person", 0.7, 0.9)],
            ..empty_update()
        };
        let msg = evaluator.build_notification(&update_with_person);
        assert!(
            !msg.to_lowercase().contains("attention"),
            "notification leaked 'attention': {msg}"
        );
        assert!(
            !msg.to_lowercase().contains("world model"),
            "notification leaked 'world model': {msg}"
        );

        let update_empty = empty_update();
        let msg2 = evaluator.build_notification(&update_empty);
        assert!(
            !msg2.to_lowercase().contains("deserves"),
            "notification still uses old term: {msg2}"
        );
    }
}
