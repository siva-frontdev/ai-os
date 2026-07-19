use memory_core::*;

#[test]
fn test_memory_object_lifecycle() {
    // Create a memory object
    let obj = MemoryObject::builder()
        .created_by("test-agent")
        .source(MemorySource::Agent)
        .memory_type(MemoryType::Episodic)
        .content_type("text/plain")
        .content(b"user said hello".to_vec())
        .tag("conversation")
        .tag("greeting")
        .importance(MemoryImportance::new(0.8).unwrap())
        .priority(MemoryPriority::HIGH)
        .build();

    assert!(obj.validate().is_ok());

    // Verify auto-generated fields
    assert_eq!(obj.version, Version::INITIAL);
    assert_eq!(obj.created_by, "test-agent");
    assert_eq!(obj.source, MemorySource::Agent);

    // Verify checksum
    let expected_cs = Checksum::compute(b"user said hello");
    assert_eq!(obj.checksum, *expected_cs.as_bytes());

    // Serialize and deserialize
    let json = serde_json::to_string(&obj).unwrap();
    let restored: MemoryObject = serde_json::from_str(&json).unwrap();
    assert_eq!(obj.id, restored.id);
    assert_eq!(obj.content_type, restored.content_type);
    assert_eq!(obj.tags, restored.tags);
}

#[test]
fn test_relationship_graph() {
    let obj_a = MemoryObject::builder()
        .content_type("text")
        .content(b"fact A".to_vec())
        .build();

    let obj_b = MemoryObject::builder()
        .content_type("text")
        .content(b"fact B (derived from A)".to_vec())
        .relationships(vec![Relationship::new(
            obj_a.id,
            RelationType::DerivesFrom,
            1.0,
        )])
        .build();

    assert!(obj_a.validate().is_ok());
    assert!(obj_b.validate().is_ok());
    assert_eq!(obj_b.relationships.len(), 1);
    assert_eq!(obj_b.relationships[0].target_id, obj_a.id);
    assert_eq!(
        obj_b.relationships[0].relation_type,
        RelationType::DerivesFrom
    );
}

#[test]
fn test_query_filter_serialization() {
    let filter = QueryFilter {
        tier: Some(MemoryTier::Working),
        memory_type: Some(MemoryType::Episodic),
        tags: Some(vec!["important".into(), "urgent".into()]),
        limit: 10,
        ..Default::default()
    };

    let json = serde_json::to_string(&filter).unwrap();
    let restored: QueryFilter = serde_json::from_str(&json).unwrap();
    assert_eq!(filter.tier, restored.tier);
    assert_eq!(filter.memory_type, restored.memory_type);
    assert_eq!(filter.tags, restored.tags);
    assert_eq!(filter.limit, restored.limit);
}

#[test]
fn test_memory_event_serialization() {
    let id = MemoryId::new();
    let event = MemoryEvent::ObjectStored {
        id,
        memory_type: MemoryType::Working,
        tier: MemoryTier::Working,
        timestamp: Timestamp::now(),
    };

    let json = serde_json::to_string(&event).unwrap();
    let restored: MemoryEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(event.event_type(), restored.event_type());
}

#[test]
fn test_version_tracking() {
    let mut v = Version::INITIAL;
    assert_eq!(v.get(), 1);
    v = v.next();
    assert_eq!(v.get(), 2);
    v = v.next();
    assert_eq!(v.get(), 3);
}

#[test]
fn test_timestamp_ordering() {
    let early = Timestamp::from_nanos(1000);
    let late = Timestamp::from_nanos(2000);
    assert!(early < late);
    assert!(late > early);
    assert_eq!(early, Timestamp::from_nanos(1000));
}

#[test]
fn test_metadata_from_iterator() {
    let meta: Metadata = vec![("key1", "value1"), ("key2", "value2"), ("key3", "value3")]
        .into_iter()
        .collect();

    assert_eq!(meta.len(), 3);
    assert_eq!(meta.get("key1"), Some("value1"));
    assert_eq!(meta.get("key2"), Some("value2"));
    assert_eq!(meta.get("key3"), Some("value3"));
}
