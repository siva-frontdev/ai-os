#[cfg(test)]
mod tests {
    use crate::dag::GoalDag;
    use crate::errors::GoalsError;
    use crate::manager::GoalManager;
    use crate::memory_store::InMemoryGoalStore;
    use crate::store::GoalStore;
    use crate::types::GoalRecord;
    use crate::validator::GoalValidator;
    use brain_core::ids::GoalId;
    use brain_core::types::GoalPriority;
    use std::sync::Arc;
    use uuid::Uuid;

    fn make_id(n: u8) -> GoalId {
        let mut buf = [0u8; 16];
        buf[0] = n;
        buf[15] = n;
        GoalId::from(Uuid::from_bytes(buf))
    }

    #[test]
    fn test_dag_new() {
        let dag = GoalDag::new();
        assert_eq!(dag.node_count(), 0);
        assert_eq!(dag.edge_count(), 0);
    }

    #[test]
    fn test_dag_add_remove_node() {
        let mut dag = GoalDag::new();
        let a = make_id(1);
        let b = make_id(2);
        dag.add_node(a);
        dag.add_node(b);
        assert_eq!(dag.node_count(), 2);
        assert!(dag.has_node(&a));
        dag.remove_node(&a);
        assert!(!dag.has_node(&a));
        assert_eq!(dag.node_count(), 1);
    }

    #[test]
    fn test_dag_add_dependency() {
        let mut dag = GoalDag::new();
        let a = make_id(1);
        let b = make_id(2);
        dag.add_node(a);
        dag.add_node(b);
        assert!(dag.add_dependency(a, b).is_ok());
        assert_eq!(dag.edge_count(), 1);
        assert!(dag.dependencies(&a).contains(&b));
        assert!(dag.dependents(&b).contains(&a));
    }

    #[test]
    fn test_dag_self_dependency() {
        let mut dag = GoalDag::new();
        let a = make_id(1);
        dag.add_node(a);
        let err = dag.add_dependency(a, a).unwrap_err();
        assert!(matches!(err, GoalsError::ValidationError(_)));
    }

    #[test]
    fn test_dag_cycle_detection() {
        let mut dag = GoalDag::new();
        let a = make_id(1);
        let b = make_id(2);
        let c = make_id(3);
        dag.add_node(a);
        dag.add_node(b);
        dag.add_node(c);
        assert!(dag.add_dependency(a, b).is_ok());
        assert!(dag.add_dependency(b, c).is_ok());
        let err = dag.add_dependency(c, a).unwrap_err();
        assert!(matches!(err, GoalsError::CycleDetected(_)));
    }

    #[test]
    fn test_dag_topological_sort_simple() {
        let mut dag = GoalDag::new();
        let a = make_id(1);
        let b = make_id(2);
        let c = make_id(3);
        dag.add_node(a);
        dag.add_node(b);
        dag.add_node(c);
        dag.add_dependency(a, b).unwrap();
        dag.add_dependency(b, c).unwrap();
        let sorted = dag.topological_sort().unwrap();
        assert_eq!(sorted.len(), 3);
    }

    #[test]
    fn test_dag_is_reachable() {
        let mut dag = GoalDag::new();
        let a = make_id(1);
        let b = make_id(2);
        let c = make_id(3);
        dag.add_node(a);
        dag.add_node(b);
        dag.add_node(c);
        dag.add_dependency(a, b).unwrap();
        dag.add_dependency(b, c).unwrap();
        assert!(dag.is_reachable(&a, &c));
        assert!(!dag.is_reachable(&c, &a));
    }

    #[test]
    fn test_validator_validate_new() {
        let v = GoalValidator::new();
        let record = GoalRecord::new(make_id(1), "test", "description", GoalPriority::Normal);
        let r = v.validate_new(&record, &[]);
        assert!(r.is_ok());
    }

    #[test]
    fn test_validator_rejects_empty_description() {
        let v = GoalValidator::new();
        let record = GoalRecord::new(make_id(1), "test", "", GoalPriority::Normal);
        let r = v.validate_new(&record, &[]);
        assert!(r.is_err());
    }

    #[test]
    fn test_validator_rejects_duplicate() {
        let v = GoalValidator::new();
        let id = make_id(1);
        let record = GoalRecord::new(id, "test", "desc", GoalPriority::Normal);
        let r = v.validate_new(&record, &[id]);
        assert!(r.is_err());
    }

    #[test]
    fn test_validator_can_activate() {
        let v = GoalValidator::new();
        let mut record = GoalRecord::new(make_id(1), "test", "desc", GoalPriority::Normal);
        record.dependencies = vec![make_id(2)];
        assert!(v.can_activate(&record, &[brain_core::types::GoalStatus::Completed]));
        assert!(!v.can_activate(&record, &[brain_core::types::GoalStatus::Pending]));
        assert!(!v.can_activate(&record, &[]));
    }

    #[tokio::test]
    async fn test_memory_store_insert_get() {
        let store = InMemoryGoalStore::new();
        let record = GoalRecord::new(make_id(1), "test", "desc", GoalPriority::Normal);
        store.insert(record.clone()).await.unwrap();
        let fetched = store.get(&make_id(1)).await.unwrap();
        assert_eq!(fetched.goal_id, record.goal_id);
        assert_eq!(fetched.description, "desc");
    }

    #[tokio::test]
    async fn test_memory_store_not_found() {
        let store = InMemoryGoalStore::new();
        let err = store.get(&make_id(99)).await.unwrap_err();
        assert!(matches!(err, GoalsError::NotFound(_)));
    }

    #[tokio::test]
    async fn test_memory_store_update() {
        let store = InMemoryGoalStore::new();
        let mut record = GoalRecord::new(make_id(1), "test", "desc", GoalPriority::Normal);
        store.insert(record.clone()).await.unwrap();
        record.description = "updated".into();
        store.update(record.clone()).await.unwrap();
        let fetched = store.get(&make_id(1)).await.unwrap();
        assert_eq!(fetched.description, "updated");
    }

    #[tokio::test]
    async fn test_memory_store_delete() {
        let store = InMemoryGoalStore::new();
        let record = GoalRecord::new(make_id(1), "test", "desc", GoalPriority::Normal);
        store.insert(record).await.unwrap();
        store.delete(&make_id(1)).await.unwrap();
        assert!(store.get(&make_id(1)).await.is_err());
    }

    #[tokio::test]
    async fn test_memory_store_list_all() {
        let store = InMemoryGoalStore::new();
        store
            .insert(GoalRecord::new(
                make_id(1),
                "a",
                "desc",
                GoalPriority::Normal,
            ))
            .await
            .unwrap();
        store
            .insert(GoalRecord::new(make_id(2), "b", "desc", GoalPriority::High))
            .await
            .unwrap();
        let all = store.list_all().await.unwrap();
        assert_eq!(all.len(), 2);
    }

    #[tokio::test]
    async fn test_goal_manager_create_and_activate() {
        let store = Arc::new(InMemoryGoalStore::new());
        let manager = GoalManager::new(store);
        let id = make_id(1);
        let goal = manager
            .create_goal(id, "test", "test description", GoalPriority::Normal, vec![])
            .await
            .unwrap();
        assert_eq!(goal.status, brain_core::types::GoalStatus::Pending);
        let activated = manager.activate_goal(&id).await.unwrap();
        assert_eq!(activated.status, brain_core::types::GoalStatus::Active);
    }

    #[tokio::test]
    async fn test_goal_manager_complete() {
        let store = Arc::new(InMemoryGoalStore::new());
        let manager = GoalManager::new(store);
        let id = make_id(1);
        manager
            .create_goal(id, "test", "desc", GoalPriority::Normal, vec![])
            .await
            .unwrap();
        manager.activate_goal(&id).await.unwrap();
        let completed = manager.complete_goal(&id, "success").await.unwrap();
        assert_eq!(completed.status, brain_core::types::GoalStatus::Completed);
    }

    #[tokio::test]
    async fn test_goal_manager_fail() {
        let store = Arc::new(InMemoryGoalStore::new());
        let manager = GoalManager::new(store);
        let id = make_id(1);
        manager
            .create_goal(id, "test", "desc", GoalPriority::Normal, vec![])
            .await
            .unwrap();
        let failed = manager
            .fail_goal(&id, "something went wrong")
            .await
            .unwrap();
        assert_eq!(failed.status, brain_core::types::GoalStatus::Failed);
    }

    #[tokio::test]
    async fn test_goal_manager_cancel() {
        let store = Arc::new(InMemoryGoalStore::new());
        let manager = GoalManager::new(store);
        let id = make_id(1);
        manager
            .create_goal(id, "test", "desc", GoalPriority::Normal, vec![])
            .await
            .unwrap();
        let cancelled = manager.cancel_goal(&id, "no longer needed").await.unwrap();
        assert_eq!(cancelled.status, brain_core::types::GoalStatus::Cancelled);
    }

    #[tokio::test]
    async fn test_goal_manager_pause_resume_path() {
        let store = Arc::new(InMemoryGoalStore::new());
        let manager = GoalManager::new(store);
        let id = make_id(1);
        manager
            .create_goal(id, "test", "desc", GoalPriority::Normal, vec![])
            .await
            .unwrap();
        manager.activate_goal(&id).await.unwrap();
        let paused = manager
            .pause_goal(&id, Some("operator request"))
            .await
            .unwrap();
        assert_eq!(paused.status, brain_core::types::GoalStatus::Paused);
    }

    #[tokio::test]
    async fn test_goal_manager_recover() {
        let store = Arc::new(InMemoryGoalStore::new());
        let manager = GoalManager::new(store);
        let id = make_id(1);
        manager
            .create_goal(id, "test", "desc", GoalPriority::Normal, vec![])
            .await
            .unwrap();
        manager.fail_goal(&id, "error").await.unwrap();
        let recovering = manager.recover_goal(&id).await.unwrap();
        assert_eq!(recovering.status, brain_core::types::GoalStatus::Recovering);
    }

    #[tokio::test]
    async fn test_goal_manager_dependencies() {
        let store = Arc::new(InMemoryGoalStore::new());
        let manager = GoalManager::new(store.clone());
        let dep_id = make_id(1);
        manager
            .create_goal(dep_id, "dep", "dependency", GoalPriority::Normal, vec![])
            .await
            .unwrap();

        let goal_id = make_id(2);
        let goal = manager
            .create_goal(
                goal_id,
                "main",
                "main goal",
                GoalPriority::High,
                vec![dep_id],
            )
            .await
            .unwrap();
        assert_eq!(goal.dependencies, vec![dep_id]);

        let result = manager.activate_goal(&goal_id).await;
        assert!(result.is_err());

        manager.complete_goal(&dep_id, "done").await.unwrap();
        let activated = manager.activate_goal(&goal_id).await.unwrap();
        assert_eq!(activated.status, brain_core::types::GoalStatus::Active);
    }
}
