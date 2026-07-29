use std::sync::Arc;

use memory_core::Timestamp;
use memory_storage::wm_store::WorldModelStore;

/// Generate a natural-language explanation of why the companion
/// remembers a particular entity.
///
/// Uses existing World Model data (importance, confidence, timestamps,
/// relationship count) to produce a human-readable explanation.
/// Never exposes internal implementation details.
pub fn explain_entity(
    store: &Arc<dyn WorldModelStore>,
    entity: &memory_core::wm::Entity,
) -> String {
    let mut reasons = Vec::new();

    // How the companion learned about this
    if entity.confidence > 0.8 {
        reasons.push(format!(
            "I'm quite sure about {} — you've mentioned it multiple times.",
            entity.name
        ));
    } else if entity.confidence > 0.5 {
        reasons.push(format!(
            "I've seen {} mentioned a few times and I'm building my understanding.",
            entity.name
        ));
    } else {
        reasons.push(format!(
            "I'm still learning about {} — I don't know much yet.",
            entity.name
        ));
    }

    // How important it is
    if entity.importance > 0.8 {
        reasons.push(format!(
            "This seems very important to you, so I pay extra attention."
        ));
    } else if entity.importance > 0.5 {
        reasons.push(format!("This is moderately important in your life."));
    }

    // Recency
    let days_ago = days_since(entity.updated_at);
    if days_ago < 1 {
        reasons.push(format!("We talked about this very recently."));
    } else if days_ago < 3 {
        reasons.push(format!("We discussed this a couple of days ago."));
    } else if days_ago < 14 {
        reasons.push(format!(
            "It's been about {days_ago} days since we last talked about this."
        ));
    } else {
        reasons.push(format!(
            "I haven't heard about this in a while ({days_ago} days)."
        ));
    }

    // Relationships with other entities
    let rel_count =
        futures::executor::block_on(store.get_relationships_for_entity(&entity.id)).len();
    if rel_count > 5 {
        reasons.push(format!(
            "This is connected to {rel_count} other things I know about you."
        ));
    } else if rel_count > 0 {
        reasons.push(format!(
            "This relates to {rel_count} other areas of your life."
        ));
    }

    reasons.join(" ")
}

fn days_since(ts: Timestamp) -> u64 {
    let now = Timestamp::now();
    let secs = now.as_secs().saturating_sub(ts.as_secs());
    (secs / 86400) as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use memory_core::wm::Entity;
    use memory_storage::wm_store::InMemoryWorldModelStore;

    #[tokio::test]
    async fn test_explain_high_confidence_recent() {
        let store: Arc<dyn WorldModelStore> = Arc::new(InMemoryWorldModelStore::new());
        let entity = Entity::new("project", "AI-OS")
            .with_importance(0.9)
            .with_confidence(0.95);
        let explanation = explain_entity(&store, &entity);
        assert!(explanation.contains("quite sure"));
        assert!(explanation.contains("very important"));
        assert!(explanation.contains("recently") || explanation.contains("days"));
    }

    #[test]
    fn test_explain_low_confidence() {
        let store: Arc<dyn WorldModelStore> = Arc::new(InMemoryWorldModelStore::new());
        let mut entity = Entity::new("skill", "Rust")
            .with_importance(0.4)
            .with_confidence(0.3);
        // Set a timestamp in the past
        entity.updated_at = Timestamp::from_secs(100);
        let explanation = explain_entity(&store, &entity);
        assert!(explanation.contains("still learning"));
        assert!(!explanation.contains("very important"));
    }

    #[tokio::test]
    async fn test_explain_with_relationships() {
        let store: Arc<dyn WorldModelStore> = Arc::new(InMemoryWorldModelStore::new());
        let entity = Entity::new("person", "Alice").with_importance(0.7);
        let entity_id = store.insert_entity(entity).await;

        let bob = Entity::new("person", "Bob");
        let bob_id = store.insert_entity(bob).await;

        store
            .insert_relationship(&memory_core::wm::Relationship::new(
                "knows",
                entity_id.clone(),
                bob_id,
            ))
            .await;

        let stored = store.get_entity(&entity_id).await.unwrap();
        let explanation = explain_entity(&store, &stored);
        assert!(explanation.contains("relates to"));
    }

    #[test]
    fn test_explain_never_mentions_internal_terms() {
        let store: Arc<dyn WorldModelStore> = Arc::new(InMemoryWorldModelStore::new());
        let entity = Entity::new("project", "Test")
            .with_importance(0.5)
            .with_confidence(0.5);
        let explanation = explain_entity(&store, &entity);

        let forbidden = [
            "world model",
            "entity",
            "confidence",
            "importance",
            "relationship",
            "strength",
            "attention",
            "cognitive loop",
        ];
        let lower = explanation.to_lowercase();
        for term in &forbidden {
            assert!(
                !lower.contains(term),
                "explanation leaked '{term}': {explanation}"
            );
        }
    }
}
