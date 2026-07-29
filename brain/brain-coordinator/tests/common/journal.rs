use brain_core::types::Decision;

use super::metrics::EvaluationReport;
use super::simulation::DayTrace;

/// Generate a human-readable journal from the simulation trace.
pub fn generate_journal(trace: &[DayTrace]) -> String {
    let mut lines = Vec::new();

    for day in trace {
        lines.push(format!(
            "\n─── Day {} ─────────────────────────────────────",
            day.day
        ));

        if let Some(ref obs) = day.observation {
            lines.push(format!("  User: \"{obs}\""));

            if let Some(Decision::Communicate { message, .. }) = &day.cycle_decision {
                lines.push(format!("  → Communicate: \"{message}\""));
            } else if let Some(Decision::UpdateMemory { entity_name, .. }) = &day.cycle_decision {
                lines.push(format!("  → UpdateMemory: {entity_name}"));
            } else if let Some(Decision::Execute { .. }) = &day.cycle_decision {
                lines.push(format!("  → Execute action"));
            } else {
                lines.push("  → Wait (silent)".into());
            }
        } else {
            lines.push("  (no user input)".into());
        }

        match &day.tick_decision {
            Decision::Communicate { message, .. } => {
                lines.push(format!("  Tick → Communicate: \"{message}\""));
            }
            Decision::UpdateMemory { entity_name, .. } => {
                lines.push(format!("  Tick → UpdateMemory: {entity_name}"));
            }
            Decision::Execute { .. } => {
                lines.push(format!("  Tick → Execute"));
            }
            Decision::Wait => {
                lines.push("  Tick → Wait (silent)".into());
            }
        }

        lines.push(format!(
            "  Context: {}",
            if day.context_entity_names.is_empty() {
                "(none)".into()
            } else {
                day.context_entity_names.join(", ")
            }
        ));

        if day.reflection_recorded {
            lines.push("  ✓ Reflection recorded".into());
        }
    }

    lines.join("\n")
}

/// Generate a summary report from the evaluation results.
pub fn generate_summary(report: &EvaluationReport) -> String {
    let mut lines = Vec::new();

    lines.push("═".repeat(60));
    lines.push(format!(
        "  Companion Evaluation Report: {}",
        report.scenario_name
    ));
    lines.push("═".repeat(60));
    lines.push(format!("  Days simulated:      {}", report.scenario_days));
    lines.push(format!("  Total cycles:        {}", report.total_cycles));
    lines.push(format!(
        "  Overall companion score: {:.2} / 1.00",
        report.overall_score()
    ));
    lines.push(String::new());
    lines.push("  ── Component Scores ──".into());
    lines.push(format!(
        "  Continuity:          {:.2}/1.00",
        report.continuity_score
    ));
    lines.push(format!(
        "  Relevance:           {:.2}/1.00",
        report.relevance_score
    ));
    lines.push(format!(
        "  Timeliness:          {:.2}/1.00",
        report.timeliness_score
    ));
    lines.push(format!(
        "  Adaptability:        {:.2}/1.00",
        report.adaptability_score
    ));
    lines.push(format!(
        "  Memory quality:      {:.2}/1.00",
        report.memory_quality_score
    ));
    lines.push(format!(
        "  Interruption quality:{:.2}/1.00",
        report.interruption_quality_score
    ));

    let weaknesses = report.weaknesses();
    if weaknesses.is_empty() {
        lines.push(String::new());
        lines.push("  ✓ No weaknesses detected.".into());
    } else {
        lines.push(String::new());
        lines.push(format!("  ⚠ Weaknesses: {}", weaknesses.join(", ")));
    }

    if !report.continuity_details.is_empty() {
        lines.push(String::new());
        lines.push("  ── Continuity Details ──".into());
        for detail in &report.continuity_details {
            lines.push(format!("    {detail}"));
        }
    }

    if !report.timeliness_details.is_empty() {
        lines.push(String::new());
        lines.push("  ── Timeliness Details ──".into());
        for detail in &report.timeliness_details {
            lines.push(format!("    {detail}"));
        }
    }

    if !report.interruption_quality_details.is_empty() {
        lines.push(String::new());
        lines.push("  ── Interruption Details ──".into());
        for detail in &report.interruption_quality_details {
            lines.push(format!("    {detail}"));
        }
    }

    lines.push("═".repeat(60));

    lines.join("\n")
}
