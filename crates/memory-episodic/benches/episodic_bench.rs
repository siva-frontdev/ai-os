use criterion::{Criterion, criterion_group, criterion_main};
use memory_core::Timestamp;
use memory_episodic::{EpisodicEvent, ExperienceRecorder, InMemoryExperienceRecorder};

pub fn bench_record_event(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let recorder = InMemoryExperienceRecorder::new();
    c.bench_function("episodic_record_event", |ben| {
        ben.to_async(&rt).iter(|| async {
            let event = EpisodicEvent {
                id: uuid::Uuid::now_v7(),
                session_id: uuid::Uuid::new_v4(),
                event_type: "bench.event".into(),
                timestamp: Timestamp::now(),
                duration_ms: Some(10),
                participants: vec![],
                location: None,
                sequence_id: None,
                content: vec![],
                metadata: std::collections::HashMap::new(),
                importance: 0.7,
                confidence: 0.9,
            };
            recorder.record(event).await.unwrap();
        })
    });
}

pub fn bench_consolidation_score(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let recorder = InMemoryExperienceRecorder::new();
    let event = EpisodicEvent {
        id: uuid::Uuid::now_v7(),
        session_id: uuid::Uuid::new_v4(),
        event_type: "t".into(),
        timestamp: Timestamp::now(),
        duration_ms: None,
        participants: vec![],
        location: None,
        sequence_id: None,
        content: vec![],
        metadata: std::collections::HashMap::new(),
        importance: 0.5,
        confidence: 0.9,
    };
    c.bench_function("episodic_score_importance", |ben| {
        ben.to_async(&rt).iter(|| async {
            recorder.score_importance(&event).await.unwrap();
        })
    });
}

criterion_group!(benches, bench_record_event, bench_consolidation_score);
criterion_main!(benches);
