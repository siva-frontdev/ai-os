use brain_core::types::Decision;

use super::simulation::DayTrace;

/// Companion evaluation report.
#[derive(Debug, Clone)]
pub struct EvaluationReport {
    pub scenario_name: String,
    pub scenario_days: usize,
    pub total_cycles: u64,

    /// Continuity: does context reference past entities?
    pub continuity_score: f64,
    pub continuity_details: Vec<String>,

    /// Relevance: are responses relevant to the current topic?
    pub relevance_score: f64,
    pub relevance_details: Vec<String>,

    /// Timeliness: does the AI reach out at appropriate times?
    pub timeliness_score: f64,
    pub timeliness_details: Vec<String>,

    /// Adaptability: does the AI adjust to changing priorities?
    pub adaptability_score: f64,
    pub adaptability_details: Vec<String>,

    /// Memory quality: are entities persisted across days?
    pub memory_quality_score: f64,
    pub memory_quality_details: Vec<String>,

    /// Interruption quality: does the AI avoid interrupting during silence?
    pub interruption_quality_score: f64,
    pub interruption_quality_details: Vec<String>,
}

impl EvaluationReport {
    /// Overall companion score (0.0–1.0), weighted average of all components.
    pub fn overall_score(&self) -> f64 {
        let weights = [
            (self.continuity_score, 0.25),
            (self.relevance_score, 0.20),
            (self.timeliness_score, 0.15),
            (self.adaptability_score, 0.15),
            (self.memory_quality_score, 0.15),
            (self.interruption_quality_score, 0.10),
        ];
        let total: f64 = weights.iter().map(|(s, w)| s * w).sum();
        total / weights.iter().map(|(_, w)| w).sum::<f64>()
    }

    /// Weaknesses — dimensions scoring below 0.6.
    pub fn weaknesses(&self) -> Vec<&'static str> {
        let mut w = Vec::new();
        if self.continuity_score < 0.6 {
            w.push("Continuity");
        }
        if self.relevance_score < 0.6 {
            w.push("Relevance");
        }
        if self.timeliness_score < 0.6 {
            w.push("Timeliness");
        }
        if self.adaptability_score < 0.6 {
            w.push("Adaptability");
        }
        if self.memory_quality_score < 0.6 {
            w.push("Memory quality");
        }
        if self.interruption_quality_score < 0.6 {
            w.push("Interruption quality");
        }
        w
    }
}

/// Evaluate companion behavior from the simulation trace.
pub fn evaluate(trace: &[DayTrace], scenario_name: &str) -> EvaluationReport {
    let total_cycles = trace.len() as u64;

    let (continuity_score, continuity_details) = eval_continuity(trace);
    let (relevance_score, relevance_details) = eval_relevance(trace);
    let (timeliness_score, timeliness_details) = eval_timeliness(trace);
    let (adaptability_score, adaptability_details) = eval_adaptability(trace);
    let (memory_quality_score, memory_quality_details) = eval_memory_quality(trace);
    let (interruption_quality_score, interruption_quality_details) =
        eval_interruption_quality(trace);

    EvaluationReport {
        scenario_name: scenario_name.to_string(),
        scenario_days: trace.len(),
        total_cycles,
        continuity_score,
        continuity_details,
        relevance_score,
        relevance_details,
        timeliness_score,
        timeliness_details,
        adaptability_score,
        adaptability_details,
        memory_quality_score,
        memory_quality_details,
        interruption_quality_score,
        interruption_quality_details,
    }
}

// ── Continuity ───────────────────────────────────────────────

/// Measures whether the AI references past entities in its responses.
/// A day's tick decision mentions context entities → continuity signal.
fn eval_continuity(trace: &[DayTrace]) -> (f64, Vec<String>) {
    let mut details = Vec::new();
    let mut total_signals = 0u64;
    let mut positive = 0u64;

    for day in trace {
        // Check tick decisions that Communicate
        if let Decision::Communicate { message, .. } = &day.tick_decision {
            total_signals += 1;
            let mentions_context = !day.context_entity_names.is_empty()
                && day
                    .context_entity_names
                    .iter()
                    .any(|name| message.contains(name.as_str()));
            if mentions_context {
                positive += 1;
                details.push(format!(
                    "Day {}: tick mentioned context entities ({})",
                    day.day,
                    day.context_entity_names.join(", ")
                ));
            } else {
                details.push(format!(
                    "Day {}: tick communicated but did not mention context",
                    day.day
                ));
            }
        }

        // Check cycle decisions that Communicate
        if let Some(Decision::Communicate { message, .. }) = &day.cycle_decision {
            total_signals += 1;
            if !day.context_entity_names.is_empty()
                && day
                    .context_entity_names
                    .iter()
                    .any(|name| message.contains(name.as_str()))
            {
                positive += 1;
            }
        }
    }

    let score = if total_signals == 0 {
        0.5 // neutral if no communication happened
    } else {
        positive as f64 / total_signals as f64
    };

    (score, details)
}

// ── Relevance ────────────────────────────────────────────────

/// Measures whether cycle decisions match the entities in the understanding.
fn eval_relevance(trace: &[DayTrace]) -> (f64, Vec<String>) {
    let mut details = Vec::new();
    let mut total = 0u64;
    let mut relevant = 0u64;

    for day in trace {
        if let Some(Decision::Communicate { message, .. }) = &day.cycle_decision {
            total += 1;
            if message.contains("noted") || message.contains("Noted") {
                // Generic response — minimal relevance
                details.push(format!("Day {}: generic 'noted' response", day.day));
                continue;
            }
            relevant += 1;
            if !day.context_entity_names.is_empty() {
                details.push(format!("Day {}: relevant response with context", day.day));
            }
        }
    }

    let score = if total == 0 {
        0.5
    } else {
        relevant as f64 / total as f64
    };

    (score, details)
}

// ── Timeliness ───────────────────────────────────────────────

/// Measures whether the AI reaches out when there's stagnation/something to say,
/// and stays silent when nothing is happening.
fn eval_timeliness(trace: &[DayTrace]) -> (f64, Vec<String>) {
    let mut details = Vec::new();
    let mut appropriate = 0u64;
    let mut total_signals = 0u64;

    for day in trace.iter() {
        let has_context = !day.context_entity_names.is_empty();
        let is_communicating = matches!(day.tick_decision, Decision::Communicate { .. });

        // If there's context and no observation, communicating is appropriate
        if day.observation.is_none() {
            total_signals += 1;
            if has_context && is_communicating {
                appropriate += 1;
                details.push(format!(
                    "Day {}: timely check-in (context present)",
                    day.day
                ));
            } else if !has_context && !is_communicating {
                appropriate += 1;
                details.push(format!("Day {}: appropriate silence (no context)", day.day));
            } else if has_context && !is_communicating {
                details.push(format!(
                    "Day {}: missed check-in opportunity (context present but silent)",
                    day.day
                ));
            } else {
                details.push(format!("Day {}: communicated without context", day.day));
            }
        }
    }

    let score = if total_signals == 0 {
        0.5
    } else {
        appropriate as f64 / total_signals as f64
    };

    (score, details)
}

// ── Adaptability ─────────────────────────────────────────────

/// Measures whether the AI's context shifts when priorities change.
fn eval_adaptability(trace: &[DayTrace]) -> (f64, Vec<String>) {
    let mut details = Vec::new();

    if trace.len() < 3 {
        return (0.5, vec!["Not enough days to evaluate adaptability".into()]);
    }

    // Check that context entity names change over time
    let mut changes = 0u64;
    let mut opportunities = 0u64;

    for window in trace.windows(2) {
        let prev = &window[0];
        let curr = &window[1];

        if !prev.context_entity_names.is_empty() && !curr.context_entity_names.is_empty() {
            opportunities += 1;
            let prev_set: std::collections::HashSet<&str> = prev
                .context_entity_names
                .iter()
                .map(|s| s.as_str())
                .collect();
            let curr_set: std::collections::HashSet<&str> = curr
                .context_entity_names
                .iter()
                .map(|s| s.as_str())
                .collect();

            // If an entity was in prev but not curr, that's an adaptability signal
            for entity in &prev_set {
                if !curr_set.contains(entity) {
                    changes += 1;
                    details.push(format!(
                        "Day {} → {}: entity '{entity}' dropped from context",
                        prev.day, curr.day
                    ));
                }
            }
        }
    }

    let score = if opportunities == 0 {
        0.5
    } else {
        (changes as f64 / opportunities as f64).min(1.0)
    };

    (score, details)
}

// ── Memory quality ───────────────────────────────────────────

/// Measures that entities survive across multiple days in the World Model.
fn eval_memory_quality(trace: &[DayTrace]) -> (f64, Vec<String>) {
    let mut details = Vec::new();

    if trace.len() < 2 {
        return (0.5, vec!["Not enough days".into()]);
    }

    let first_day = &trace[0];
    let last_day = &trace[trace.len() - 1];

    // Count entities present on both first and last day
    let first_entities: std::collections::HashSet<&str> = first_day
        .context_entity_names
        .iter()
        .map(|s| s.as_str())
        .collect();
    let last_entities: std::collections::HashSet<&str> = last_day
        .context_entity_names
        .iter()
        .map(|s| s.as_str())
        .collect();

    if first_entities.is_empty() {
        return (0.7, vec!["No entities on first day to track".into()]);
    }

    let retained: Vec<&&str> = first_entities
        .iter()
        .filter(|e| last_entities.contains(*e))
        .collect();
    let score = retained.len() as f64 / first_entities.len() as f64;

    if retained.is_empty() && !first_entities.is_empty() {
        details.push("All entities from day 1 were lost by the final day".into());
    } else if !retained.is_empty() {
        details.push(format!(
            "Retained {}/{} entities from day 1 to final day",
            retained.len(),
            first_entities.len()
        ));
    }

    (score, details)
}

// ── Interruption quality ─────────────────────────────────────

/// Measures that the AI doesn't interrupt during silent/low-activity periods.
fn eval_interruption_quality(trace: &[DayTrace]) -> (f64, Vec<String>) {
    let mut details = Vec::new();

    // Find consecutive silent periods (2+ days with no observation)
    let mut silent_streaks: Vec<u64> = Vec::new();
    let mut current_streak = 0u64;
    let mut streak_start = 0u64;

    for day in trace {
        if day.observation.is_none() {
            if current_streak == 0 {
                streak_start = day.day;
            }
            current_streak += 1;
        } else {
            if current_streak >= 2 {
                silent_streaks.push(current_streak);
                // Check if AI interrupted during this streak
                let streak_days: Vec<&DayTrace> = trace
                    .iter()
                    .filter(|d| d.day >= streak_start && d.day < streak_start + current_streak)
                    .collect();
                let interruptions = streak_days
                    .iter()
                    .filter(|d| matches!(d.tick_decision, Decision::Communicate { .. }))
                    .count();
                if interruptions > 0 {
                    details.push(format!(
                        "Streak (days {}-{}): {} interruptions during {} days of silence",
                        streak_start,
                        streak_start + current_streak - 1,
                        interruptions,
                        current_streak
                    ));
                } else {
                    details.push(format!(
                        "Streak (days {}-{}): appropriate silence",
                        streak_start,
                        streak_start + current_streak - 1,
                    ));
                }
            }
            current_streak = 0;
        }
    }

    if silent_streaks.is_empty() {
        return (0.7, vec!["No silent streaks to evaluate".into()]);
    }

    let total_interruptions: usize = trace
        .iter()
        .filter(|d| {
            d.observation.is_none() && matches!(d.tick_decision, Decision::Communicate { .. })
        })
        .count();
    let silent_days: usize = trace.iter().filter(|d| d.observation.is_none()).count();

    // Some interruptions during silence are OK (timely check-ins),
    // but ideally not every silent day triggers a communication
    let score = if silent_days == 0 {
        0.7
    } else {
        let interruption_ratio = total_interruptions as f64 / silent_days as f64;
        // Perfect = 0 interruptions; acceptable = up to 30% of silent days
        (1.0 - interruption_ratio * 0.7).clamp(0.0, 1.0)
    };

    (score, details)
}
