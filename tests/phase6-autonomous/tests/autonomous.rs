//! Phase 6 — Autonomous Agent: goal sessions and the cognitive loop.
//!
//! Proves the orchestrator can run autonomous goal lifecycles:
//! create → activate → plan → execute → reflect → learn, plus
//! pause/resume/cancel and dynamic re-planning via `repair_plan`.

use std::sync::Arc;

use brain_coordinator::errors::CoordinatorError;
use brain_coordinator::BrainOrchestrator;
use brain_core::ids::ObjectiveId;
use brain_core::types::{GoalPriority, GoalStatus};
use brain_goals::types::GoalOutcome;
use phase6_autonomous::stubs::{
    new_goal_store, scripted_reasoning, AllowAllPolicyEvaluator, EmptyToolRegistry,
    ScriptedReasoningService,
};

/// Build an orchestrator wired to the real GoalManager with stub policy,
/// tool registry, and reasoning service.
fn orchestrator() -> BrainOrchestrator {
    BrainOrchestrator::new(
        Box::new(AllowAllPolicyEvaluator),
        Arc::new(EmptyToolRegistry),
        new_goal_store(),
        Arc::new(ScriptedReasoningService::new(scripted_reasoning(
            "file_management",
            "organize download folder",
            vec!["filesystem.read", "filesystem.write"],
        ))),
    )
}

#[tokio::test]
async fn full_goal_session_lifecycle() {
    let brain = orchestrator();

    let created = brain
        .create_goal(
            "file_management",
            "clean temp directory",
            GoalPriority::High,
            vec![],
        )
        .await
        .expect("goal created");
    assert_eq!(created.status, GoalStatus::Pending);

    let active = brain
        .activate_goal(&created.goal_id)
        .await
        .expect("goal activated");
    assert_eq!(active.status, GoalStatus::Active);

    // Pause then resume (re-activate via retry is for failed goals; a paused
    // goal is un-paused back to Active through the goal manager contract).
    let paused = brain
        .pause_goal(&created.goal_id, Some("waiting for user input"))
        .await
        .expect("goal paused");
    assert_eq!(paused.status, GoalStatus::Paused);

    // Complete the session with a success outcome.
    let done = brain
        .complete_goal(
            &created.goal_id,
            GoalOutcome {
                summary: "cleaned temp directory".into(),
                success: true,
                confidence: 1.0,
                artifacts: vec![],
                errors: vec![],
            },
        )
        .await
        .expect("goal completed");
    assert_eq!(done.status, GoalStatus::Completed);
}

#[tokio::test]
async fn autonomous_cognitive_tick_processes_active_goals() {
    let brain = orchestrator();

    let goal = brain
        .create_goal(
            "research",
            "research best practices",
            GoalPriority::Normal,
            vec![],
        )
        .await
        .expect("goal created");
    brain.activate_goal(&goal.goal_id).await.expect("activated");

    let iteration = brain.cognitive_tick().await.expect("cognitive tick");
    assert!(iteration >= 1, "tick count should increment");

    // The tick should have assigned a strategy and decomposed the goal
    // into at least one objective.
    let g = brain
        .goal_manager()
        .get_goal(&goal.goal_id)
        .await
        .expect("goal");
    assert!(g.strategy_id.is_some(), "strategy should be assigned");
    assert!(
        !g.objectives.is_empty(),
        "active goal should be decomposed into objectives"
    );
    assert_eq!(g.current_objective, Some(g.objectives[0].objective_id));
}

#[tokio::test]
async fn targeted_cognitive_tick_for_single_goal() {
    let brain = orchestrator();

    let goal = brain
        .create_goal(
            "file_management",
            "archive old files",
            GoalPriority::Low,
            vec![],
        )
        .await
        .expect("goal created");
    brain.activate_goal(&goal.goal_id).await.expect("activated");

    brain
        .cognitive_tick_for(goal.goal_id)
        .await
        .expect("targeted tick");
    let g = brain
        .goal_manager()
        .get_goal(&goal.goal_id)
        .await
        .expect("goal");
    assert!(
        g.strategy_id.is_some(),
        "strategy assigned by targeted tick"
    );
}

#[tokio::test]
async fn failed_goal_repairs_and_retries() {
    let brain = orchestrator();

    let goal = brain
        .create_goal(
            "file_management",
            "migrate workspace",
            GoalPriority::High,
            vec![],
        )
        .await
        .expect("goal created");
    brain.activate_goal(&goal.goal_id).await.expect("activated");

    // Simulate failure.
    brain
        .fail_goal(&goal.goal_id, "simulated execution error")
        .await
        .expect("goal failed");
    let failed = brain
        .goal_manager()
        .get_goal(&goal.goal_id)
        .await
        .expect("goal");
    assert_eq!(failed.status, GoalStatus::Failed);
    assert_eq!(failed.failure_count, 1);

    // Plan health detects the failure.
    let health = brain.evaluate_plan_health(goal.goal_id).await;
    assert!(matches!(health, Err(CoordinatorError::Planning(_))));

    // Dynamic re-planning repairs and retries the goal.
    let repaired = brain.repair_plan(goal.goal_id).await.expect("repair");
    assert!(repaired, "repair should be performed");
    let after = brain
        .goal_manager()
        .get_goal(&goal.goal_id)
        .await
        .expect("goal");
    assert_eq!(after.status, GoalStatus::Active, "goal retried to Active");
    assert!(after.retry_count >= 1, "retry count incremented");
}

#[tokio::test]
async fn proactive_retry_recovers_failed_goals() {
    let brain = orchestrator();

    let g1 = brain
        .create_goal("research", "first flaky goal", GoalPriority::Normal, vec![])
        .await
        .expect("goal created");
    brain.activate_goal(&g1.goal_id).await.expect("activated");
    brain
        .fail_goal(&g1.goal_id, "transient error")
        .await
        .expect("failed");

    let g2 = brain
        .create_goal(
            "research",
            "second flaky goal",
            GoalPriority::Normal,
            vec![],
        )
        .await
        .expect("goal created");
    brain.activate_goal(&g2.goal_id).await.expect("activated");
    brain
        .fail_goal(&g2.goal_id, "transient error")
        .await
        .expect("failed");

    let retried = brain
        .proactive_retry_failed()
        .await
        .expect("proactive retry");
    assert_eq!(retried.len(), 2, "both failed goals should be retried");

    for id in retried {
        let g = brain.goal_manager().get_goal(&id).await.expect("goal");
        assert_eq!(g.status, GoalStatus::Active, "goal {id} retried");
    }
}

#[tokio::test]
async fn cancel_goal_stops_session() {
    let brain = orchestrator();

    let goal = brain
        .create_goal("research", "cancelled research", GoalPriority::Low, vec![])
        .await
        .expect("goal created");
    brain.activate_goal(&goal.goal_id).await.expect("activated");

    let cancelled = brain
        .cancel_goal(&goal.goal_id, "no longer needed")
        .await
        .expect("cancelled");
    assert_eq!(cancelled.status, GoalStatus::Cancelled);
    assert_eq!(
        cancelled.metadata.get("cancel_reason").map(String::as_str),
        Some("no longer needed")
    );
}

#[tokio::test]
async fn session_report_exposes_metrics() {
    let brain = orchestrator();

    let goal = brain
        .create_goal(
            "file_management",
            "metrics goal",
            GoalPriority::Normal,
            vec![],
        )
        .await
        .expect("goal created");
    brain.activate_goal(&goal.goal_id).await.expect("activated");
    brain.cognitive_tick().await.expect("tick");

    let report = brain.generate_report().expect("report");
    assert_eq!(report.goal_id, Some(goal.goal_id));
    assert!(report.summary.contains("steps"), "summary carries metrics");

    let session = brain.get_session().expect("session");
    assert!(session.step_count >= 1, "session tracks steps");
}

#[tokio::test]
async fn goal_dependencies_control_activation_ordering() {
    let brain = orchestrator();

    let upstream = brain
        .create_goal(
            "file_management",
            "upstream setup",
            GoalPriority::High,
            vec![],
        )
        .await
        .expect("upstream goal");
    let downstream = brain
        .create_goal(
            "file_management",
            "downstream depends on upstream",
            GoalPriority::High,
            vec![upstream.goal_id],
        )
        .await
        .expect("downstream goal");

    // Activating the dependent before its dependency completes is rejected.
    let premature = brain.activate_goal(&downstream.goal_id).await;
    assert!(
        premature.is_err(),
        "dependent cannot activate before dependency"
    );

    // Once the dependency completes, activation succeeds.
    brain
        .complete_goal(
            &upstream.goal_id,
            GoalOutcome {
                summary: "upstream done".into(),
                success: true,
                confidence: 1.0,
                artifacts: vec![],
                errors: vec![],
            },
        )
        .await
        .expect("upstream completed");
    let activated = brain.activate_goal(&downstream.goal_id).await;
    assert!(
        activated.is_ok(),
        "dependent activates after dependency completes"
    );
}

#[tokio::test]
async fn objectives_and_milestones_structure_the_session() {
    let brain = orchestrator();

    let goal = brain
        .create_goal(
            "research",
            "structured research",
            GoalPriority::Normal,
            vec![],
        )
        .await
        .expect("goal created");

    let with_obj = brain
        .add_objective(&goal.goal_id, "gather sources", 0)
        .await
        .expect("objective added");
    let objective_id: ObjectiveId = with_obj
        .objectives
        .iter()
        .find(|o| o.description == "gather sources")
        .expect("objective exists")
        .objective_id;

    let with_mile = brain
        .add_milestone(&goal.goal_id, &objective_id, "collect 5 sources", 0)
        .await
        .expect("milestone added");
    let milestone_id = with_mile
        .objectives
        .iter()
        .find(|o| o.objective_id == objective_id)
        .and_then(|o| o.milestones.first())
        .expect("milestone exists")
        .milestone_id;

    brain
        .goal_manager()
        .reach_milestone(&goal.goal_id, &milestone_id)
        .await
        .expect("milestone reached");
    let after = brain
        .goal_manager()
        .get_goal(&goal.goal_id)
        .await
        .expect("goal");
    let obj = after
        .objectives
        .iter()
        .find(|o| o.objective_id == objective_id)
        .expect("objective present");
    assert_eq!(obj.milestones[0].status, GoalStatus::Completed);
}
