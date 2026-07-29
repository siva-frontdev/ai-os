/// Life Simulation & Evaluation Framework
///
/// Runs multi-day scenarios against the CognitiveLoopService to evaluate
/// companion behavior across: continuity, relevance, timeliness, adaptability,
/// memory quality, and interruption quality.
mod common;

use common::journal;
use common::metrics;
use common::scenarios;
use common::simulation::SimulationEngine;

/// Helper: run a named scenario and return its evaluation report.
async fn evaluate_scenario(scenario: &scenarios::Scenario) -> metrics::EvaluationReport {
    // Extract responses for days that have observations
    let responses: Vec<_> = scenario
        .days
        .iter()
        .filter_map(|d| d.understanding.clone())
        .collect();

    let mut sim = SimulationEngine::new(scenario.initial_entities.clone(), responses).await;
    sim.run_scenario(&scenario.days).await;

    let trace = sim.trace().to_vec();
    let report = metrics::evaluate(&trace, scenario.name);

    // Print journal
    println!("{}", journal::generate_journal(&trace));
    // Print evaluation summary
    println!("{}", journal::generate_summary(&report));

    report
}

// ── Scenario Tests ───────────────────────────────────────────

#[tokio::test]
async fn simulation_long_term_project() {
    let scenario = scenarios::long_term_project();
    let report = evaluate_scenario(&scenario).await;
    println!("Overall companion score: {:.2}", report.overall_score());
    // The simulation must complete without error
    assert!(report.total_cycles > 0, "simulation should execute cycles");
}

#[tokio::test]
async fn simulation_learning_journey() {
    let scenario = scenarios::learning_journey();
    let report = evaluate_scenario(&scenario).await;
    println!("Overall companion score: {:.2}", report.overall_score());
    assert!(report.total_cycles > 0, "simulation should execute cycles");
}

#[tokio::test]
async fn simulation_changing_priorities() {
    let scenario = scenarios::changing_priorities();
    let report = evaluate_scenario(&scenario).await;
    println!("Overall companion score: {:.2}", report.overall_score());
    assert!(report.total_cycles > 0, "simulation should execute cycles");
}

#[tokio::test]
async fn simulation_stress_inactivity() {
    let scenario = scenarios::stress_and_inactivity();
    let report = evaluate_scenario(&scenario).await;
    println!("Overall companion score: {:.2}", report.overall_score());
    assert!(report.total_cycles > 0, "simulation should execute cycles");
}

#[tokio::test]
async fn simulation_success_failure() {
    let scenario = scenarios::success_and_failure();
    let report = evaluate_scenario(&scenario).await;
    println!("Overall companion score: {:.2}", report.overall_score());
    assert!(report.total_cycles > 0, "simulation should execute cycles");
}

#[tokio::test]
async fn simulation_interrupted_work() {
    let scenario = scenarios::interrupted_work();
    let report = evaluate_scenario(&scenario).await;
    println!("Overall companion score: {:.2}", report.overall_score());
    assert!(report.total_cycles > 0, "simulation should execute cycles");
}

#[tokio::test]
async fn simulation_all_scenarios() {
    let all_scenarios = vec![
        scenarios::long_term_project(),
        scenarios::learning_journey(),
        scenarios::changing_priorities(),
        scenarios::stress_and_inactivity(),
        scenarios::success_and_failure(),
        scenarios::interrupted_work(),
    ];

    let mut overall_scores = Vec::new();

    for scenario in &all_scenarios {
        println!("\n\n{}", "=".repeat(70));
        println!("  SCENARIO: {}", scenario.name);
        println!("  {}", scenario.description);
        println!("{}", "=".repeat(70));

        let responses: Vec<_> = scenario
            .days
            .iter()
            .filter_map(|d| d.understanding.clone())
            .collect();

        let mut sim = SimulationEngine::new(scenario.initial_entities.clone(), responses).await;
        sim.run_scenario(&scenario.days).await;

        let trace = sim.trace().to_vec();
        let report = metrics::evaluate(&trace, scenario.name);

        overall_scores.push((scenario.name, report.overall_score()));

        // Print journal summary (first 5 lines + last 3 lines for brevity)
        let journal_text = journal::generate_journal(&trace);
        let lines: Vec<&str> = journal_text.lines().collect();
        for line in lines.iter().take(5) {
            println!("{line}");
        }
        println!("  ... ({} days, {} lines total)", trace.len(), lines.len());
        for line in lines.iter().rev().take(3).rev() {
            println!("{line}");
        }

        // Print evaluation summary
        println!("{}", journal::generate_summary(&report));
    }

    // Print overall ranking
    println!("\n\n{}", "=".repeat(70));
    println!("  OVERALL RANKING");
    println!("{}", "=".repeat(70));
    overall_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    for (i, (name, score)) in overall_scores.iter().enumerate() {
        println!("  {}. {}: {:.2}", i + 1, name, score);
    }

    // Verify all scenarios ran
    assert_eq!(
        overall_scores.len(),
        6,
        "all 6 scenarios should have been evaluated"
    );
}
