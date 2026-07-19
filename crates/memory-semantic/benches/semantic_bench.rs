use criterion::{Criterion, criterion_group, criterion_main};
use memory_semantic::{InMemorySemanticMemory, SemanticMemory};

pub fn bench_concept_crud(c: &mut Criterion) {
    let mem = InMemorySemanticMemory::new();
    c.bench_function("semantic_create_concept", |b| {
        b.iter(|| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                mem.create_concept(format!("concept-{}", uuid::Uuid::new_v4()), "entity".into())
                    .await
                    .unwrap();
            });
        })
    });
}

pub fn bench_list_concepts(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mem = InMemorySemanticMemory::new();
    rt.block_on(async {
        for i in 0..100u64 {
            mem.create_concept(format!("c-{i}"), "entity".into())
                .await
                .unwrap();
        }
    });
    c.bench_function("semantic_list_concepts", |b| {
        b.iter(|| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                mem.list_concepts().await.unwrap();
            });
        })
    });
}

pub fn bench_add_fact(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mem = InMemorySemanticMemory::new();
    rt.block_on(async {
        let concept = mem
            .create_concept("subject".into(), "entity".into())
            .await
            .unwrap();
        c.bench_function("semantic_add_fact", |b| {
            b.iter(|| {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    mem.add_fact(memory_semantic::Fact {
                        id: uuid::Uuid::now_v7(),
                        subject: concept.id,
                        predicate: "has_property".into(),
                        object: "value".into(),
                        confidence: 0.9,
                        timestamp: memory_core::Timestamp::now().as_nanos() as i64,
                        support_count: 1,
                    })
                    .await
                    .unwrap();
                });
            })
        });
    });
}

criterion_group!(
    benches,
    bench_concept_crud,
    bench_list_concepts,
    bench_add_fact
);
criterion_main!(benches);
