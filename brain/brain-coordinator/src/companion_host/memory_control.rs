use std::sync::Arc;

use memory_core::wm::Entity;
use memory_storage::wm_store::WorldModelStore;

use crate::errors::CoordinatorError;

/// Forget an entity by name. Removes the entity and all its relationships.
pub async fn forget_entity(
    store: &Arc<dyn WorldModelStore>,
    name: &str,
) -> Result<String, CoordinatorError> {
    let entities = store.search_entities_by_name(name).await;
    if entities.is_empty() {
        return Err(CoordinatorError::Internal(format!(
            "No memory found matching '{name}'"
        )));
    }
    let mut deleted_names = Vec::new();
    for entity in &entities {
        store.delete_entity(&entity.id).await;
        deleted_names.push(entity.name.clone());
    }
    Ok(format!("Forgot about {}", deleted_names.join(", ")))
}

/// Forget everything — clear the entire World Model.
pub async fn forget_all(store: &Arc<dyn WorldModelStore>) -> String {
    store.clear().await;
    "All memories have been cleared.".into()
}

/// Merge two entities: merge `source_name` into `target_name`, then delete source.
pub async fn merge_entities(
    store: &Arc<dyn WorldModelStore>,
    source_name: &str,
    target_name: &str,
) -> Result<String, CoordinatorError> {
    let sources = store.search_entities_by_name(source_name).await;
    let targets = store.search_entities_by_name(target_name).await;

    let source = sources
        .first()
        .ok_or_else(|| CoordinatorError::Internal(format!("Source '{source_name}' not found")))?;
    let target = targets
        .first()
        .ok_or_else(|| CoordinatorError::Internal(format!("Target '{target_name}' not found")))?;

    // Merge confidence and importance (take the max)
    let merged = Entity {
        id: target.id.clone(),
        name: target.name.clone(),
        entity_type: target.entity_type.clone(),
        importance: target.importance.max(source.importance),
        confidence: target.confidence.max(source.confidence),
        ..target.clone()
    };
    store.update_entity(&merged).await;

    // Move relationships from source to target
    let rels = store.get_relationships_for_entity(&source.id).await;
    for rel in &rels {
        if rel.source_id == source.id {
            let mut new_rel = rel.clone();
            new_rel.source_id = target.id.clone();
            store.insert_relationship(&new_rel).await;
        }
        if rel.target_id == source.id {
            let mut new_rel = rel.clone();
            new_rel.target_id = target.id.clone();
            store.insert_relationship(&new_rel).await;
        }
    }

    store.delete_entity(&source.id).await;
    Ok(format!("Merged '{source_name}' into '{target_name}'"))
}

/// Correct an entity's name, type, or properties.
pub async fn correct_entity(
    store: &Arc<dyn WorldModelStore>,
    old_name: &str,
    new_name: Option<&str>,
    new_type: Option<&str>,
) -> Result<String, CoordinatorError> {
    let entities = store.search_entities_by_name(old_name).await;
    let entity = entities
        .first()
        .ok_or_else(|| CoordinatorError::Internal(format!("No memory found: '{old_name}'")))?;

    let corrected = Entity {
        id: entity.id.clone(),
        name: new_name.unwrap_or(&entity.name).to_string(),
        entity_type: new_type.unwrap_or(&entity.entity_type).to_string(),
        ..entity.clone()
    };
    store.update_entity(&corrected).await;

    let mut changes = Vec::new();
    if new_name.is_some() {
        changes.push(format!("renamed to '{}'", corrected.name));
    }
    if new_type.is_some() {
        changes.push(format!("type changed to '{}'", corrected.entity_type));
    }
    Ok(format!("Corrected '{old_name}': {}", changes.join(", ")))
}

/// Forget a date range: remove all entities updated between `start` and `end`
/// timestamps (Unix seconds). This is intentionally conservative — it only affects
/// entities whose `updated_at` falls within the range.
pub async fn forget_date_range(
    store: &Arc<dyn WorldModelStore>,
    start_secs: i64,
    end_secs: i64,
) -> Result<String, CoordinatorError> {
    let all = store.all_entities().await;
    let mut count = 0u64;
    for entity in &all {
        let ts = entity.updated_at.as_secs();
        if ts >= start_secs && ts <= end_secs {
            store.delete_entity(&entity.id).await;
            count += 1;
        }
    }
    if count == 0 {
        return Err(CoordinatorError::Internal(
            "No memories found in that date range".into(),
        ));
    }
    Ok(format!("Forgot {count} memories from that period"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use memory_core::Timestamp;
    use memory_storage::wm_store::InMemoryWorldModelStore;

    #[tokio::test]
    async fn test_forget_entity_by_name() {
        let store = Arc::new(InMemoryWorldModelStore::new()) as Arc<dyn WorldModelStore>;
        let entity = Entity::new("person", "Alice").with_importance(0.9);
        store.insert_entity(entity).await;
        assert_eq!(store.entity_count().await, 1);

        let result = forget_entity(&store, "Alice").await.unwrap();
        assert!(result.contains("Alice"));
        assert_eq!(store.entity_count().await, 0);
    }

    #[tokio::test]
    async fn test_forget_nonexistent_returns_error() {
        let store = Arc::new(InMemoryWorldModelStore::new()) as Arc<dyn WorldModelStore>;
        let result = forget_entity(&store, "Ghost").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_forget_all() {
        let store = Arc::new(InMemoryWorldModelStore::new()) as Arc<dyn WorldModelStore>;
        store.insert_entity(Entity::new("person", "Alice")).await;
        store.insert_entity(Entity::new("project", "AI-OS")).await;
        assert_eq!(store.entity_count().await, 2);

        let msg = forget_all(&store).await;
        assert!(msg.contains("cleared"));
        assert_eq!(store.entity_count().await, 0);
    }

    #[tokio::test]
    async fn test_correct_entity_name() {
        let store = Arc::new(InMemoryWorldModelStore::new()) as Arc<dyn WorldModelStore>;
        store
            .insert_entity(Entity::new("person", "Alic").with_importance(0.7))
            .await;

        let result = correct_entity(&store, "Alic", Some("Alice"), None)
            .await
            .unwrap();
        assert!(result.contains("Alice"));

        let entities = store.search_entities_by_name("Alice").await;
        assert_eq!(entities.len(), 1);
    }

    #[tokio::test]
    async fn test_merge_entities() {
        let store = Arc::new(InMemoryWorldModelStore::new()) as Arc<dyn WorldModelStore>;
        store
            .insert_entity(Entity::new("project", "AI OS").with_importance(0.6))
            .await;
        store
            .insert_entity(Entity::new("project", "AI-OS").with_importance(0.9))
            .await;

        let result = merge_entities(&store, "AI OS", "AI-OS").await.unwrap();
        assert!(result.contains("Merged"));

        // Source should be gone
        let sources = store.search_entities_by_name("AI OS").await;
        assert!(sources.is_empty());

        // Target should exist with merged importance
        let targets = store.search_entities_by_name("AI-OS").await;
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].importance, 0.9); // took the max
    }

    #[tokio::test]
    async fn test_forget_date_range() {
        use memory_core::Timestamp;
        let store = Arc::new(InMemoryWorldModelStore::new()) as Arc<dyn WorldModelStore>;
        let past = Timestamp::from_secs(1000);
        let recent = Timestamp::from_secs(5000);
        let now = Timestamp::now();

        let mut old_entity = Entity::new("project", "Old");
        old_entity.updated_at = past;
        store.insert_entity(old_entity).await;

        let mut current_entity = Entity::new("project", "Current");
        current_entity.updated_at = now;
        store.insert_entity(current_entity).await;

        let result = forget_date_range(&store, 100, 2000).await.unwrap();
        assert!(result.contains("1"));

        assert_eq!(store.entity_count().await, 1);
        let remaining = store.all_entities().await;
        assert_eq!(remaining[0].name, "Current");
    }
}
