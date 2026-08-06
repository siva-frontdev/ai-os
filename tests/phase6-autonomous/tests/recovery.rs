//! Phase 6 — Autonomous Agent: recovery and persistence.
//!
//! Proves the platform can detect interrupted goals, resume them across
//! restarts, and verify persistence via snapshot/restore round-trips.

use brain_coordinator::recovery::RecoveryManager;
use brain_core::ids::GoalId;
use brain_core::types::{GoalPriority, GoalStatus};
use brain_goals::types::GoalOutcome;
use brain_goals::GoalManager;
use phase6_autonomous::stubs::new_goal_store;

fn goal_manager() -> GoalManager {
    GoalManager::new(new_goal_store())
}

#[tokio::test]
async fn finds_interrupted_goals_after_simulated_restart() {
    let manager = goal_manager();

    let running = manager
        .create_goal(
            GoalId::new(),
            "research",
            "interrupted research",
            GoalPriority::High,
            vec![],
        )
        .await
        .expect("goal created");
    manager
        .activate_goal(&running.goal_id)
        .await
        .expect("activated");

    let done = manager
        .create_goal(
            GoalId::new(),
            "research",
            "finished research",
            GoalPriority::High,
            vec![],
        )
        .await
        .expect("goal created");
    manager
        .activate_goal(&done.goal_id)
        .await
        .expect("activated");
    manager
        .complete_goal(
            &done.goal_id,
            GoalOutcome {
                summary: "done".into(),
                success: true,
                confidence: 1.0,
                artifacts: vec![],
                errors: vec![],
            },
        )
        .await
        .expect("completed");

    // Simulate a restart: the store has goals but no in-memory manager state.
    let store = manager.store().clone();
    let interrupted = RecoveryManager::find_interrupted_goals(store.as_ref())
        .await
        .expect("scan");
    assert_eq!(interrupted.len(), 1, "only the active goal is interrupted");
    assert_eq!(interrupted[0].goal_id, running.goal_id);
}

#[tokio::test]
async fn resume_all_recovers_interrupted_goals() {
    let manager = goal_manager();

    let goal = manager
        .create_goal(
            GoalId::new(),
            "file_management",
            "resume me",
            GoalPriority::High,
            vec![],
        )
        .await
        .expect("goal created");
    manager
        .activate_goal(&goal.goal_id)
        .await
        .expect("activated");

    let resumed = RecoveryManager::resume_all(&manager).await.expect("resume");
    assert!(
        resumed.contains(&goal.goal_id),
        "interrupted goal should be resumed"
    );

    let after = manager.get_goal(&goal.goal_id).await.expect("goal");
    assert_eq!(
        after.status,
        GoalStatus::Recovering,
        "interrupted goal marked recovering"
    );
    assert_eq!(after.recovery_attempts, 1);
}

#[tokio::test]
async fn resume_all_with_no_interrupted_goals_is_noop() {
    let manager = goal_manager();
    let resumed = RecoveryManager::resume_all(&manager).await.expect("resume");
    assert!(resumed.is_empty(), "nothing to resume");
}

#[tokio::test]
async fn paused_goals_are_listed_but_not_resumed() {
    let manager = goal_manager();

    let goal = manager
        .create_goal(
            GoalId::new(),
            "research",
            "paused research",
            GoalPriority::Low,
            vec![],
        )
        .await
        .expect("goal created");
    manager
        .activate_goal(&goal.goal_id)
        .await
        .expect("activated");
    manager
        .pause_goal(&goal.goal_id, Some("user away"))
        .await
        .expect("paused");

    let store = manager.store().clone();
    let paused = RecoveryManager::find_paused_goals(store.as_ref())
        .await
        .expect("scan");
    assert_eq!(paused.len(), 1);

    let resumed = RecoveryManager::resume_all(&manager).await.expect("resume");
    assert!(
        resumed.contains(&goal.goal_id),
        "paused goal listed in resume output"
    );
    let after = manager.get_goal(&goal.goal_id).await.expect("goal");
    assert_eq!(after.status, GoalStatus::Paused, "paused stays paused");
}

#[tokio::test]
async fn persistence_round_trip_preserves_goals() {
    let manager = goal_manager();

    for i in 0..5 {
        manager
            .create_goal(
                GoalId::new(),
                "file_management",
                format!("persisted goal {i}"),
                GoalPriority::Normal,
                vec![],
            )
            .await
            .expect("goal created");
    }

    let ok = RecoveryManager::verify_persistence(&manager)
        .await
        .expect("persistence check");
    assert!(ok, "snapshot/restore preserves goal count");

    let all = manager.list_goals().await.expect("list");
    assert_eq!(all.len(), 5, "goals survive the round-trip");
}

#[tokio::test]
async fn goals_survive_snapshot_and_restore() {
    let manager = goal_manager();

    let goal = manager
        .create_goal(
            GoalId::new(),
            "research",
            "durable goal",
            GoalPriority::High,
            vec![],
        )
        .await
        .expect("goal created");
    manager
        .activate_goal(&goal.goal_id)
        .await
        .expect("activated");

    manager.snapshot("pre_add").await.expect("snapshot");

    // Add an unrelated goal, then restore — only the snapshot survives.
    let extra = manager
        .create_goal(
            GoalId::new(),
            "research",
            "temporary goal",
            GoalPriority::Low,
            vec![],
        )
        .await
        .expect("goal created");

    manager.restore("pre_add").await.expect("restore");

    let all = manager.list_goals().await.expect("list");
    assert_eq!(all.len(), 1, "temporary goal rolled back by restore");
    assert_eq!(all[0].goal_id, goal.goal_id);
    assert_eq!(all[0].status, GoalStatus::Active);
    assert_ne!(all[0].goal_id, extra.goal_id);
}
