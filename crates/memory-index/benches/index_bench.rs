use criterion::{Criterion, criterion_group, criterion_main};

use memory_core::{MemoryId, RelationType, Timestamp};
use memory_index::*;

fn bench_default_metadata_index(c: &mut Criterion) {
    let idx = DefaultMetadataIndex;
    let id = MemoryId::new();
    let rt = tokio::runtime::Runtime::new().expect("runtime");

    c.bench_function("DefaultMetadataIndex::index_metadata", |ben| {
        ben.to_async(&rt).iter(|| async {
            let _ = idx.index_metadata(&id, "key", "value").await;
        });
    });
}

fn bench_default_tag_index(c: &mut Criterion) {
    let idx = DefaultTagIndex;
    let id = MemoryId::new();
    let tags = vec!["a".into(), "b".into(), "c".into()];
    let rt = tokio::runtime::Runtime::new().expect("runtime");

    c.bench_function("DefaultTagIndex::index_tags", |ben| {
        ben.to_async(&rt).iter(|| async {
            let _ = idx.index_tags(&id, &tags).await;
        });
    });
}

fn bench_default_relationship_index(c: &mut Criterion) {
    let idx = DefaultRelationshipIndex;
    let id_a = MemoryId::new();
    let id_b = MemoryId::new();
    let rt = tokio::runtime::Runtime::new().expect("runtime");

    c.bench_function("DefaultRelationshipIndex::index_relationship", |ben| {
        ben.to_async(&rt).iter(|| async {
            let _ = idx.index_relationship(&id_a, &id_b, &RelationType::References).await;
        });
    });
}

fn bench_default_time_index(c: &mut Criterion) {
    let idx = DefaultTimeIndex;
    let id = MemoryId::new();
    let now = Timestamp::now();
    let rt = tokio::runtime::Runtime::new().expect("runtime");

    c.bench_function("DefaultTimeIndex::index_time", |ben| {
        ben.to_async(&rt).iter(|| async {
            let _ = idx.index_time(&id, now).await;
        });
    });
}

criterion_group!(
    name = benches;
    config = Criterion::default();
    targets =
        bench_default_metadata_index,
        bench_default_tag_index,
        bench_default_relationship_index,
        bench_default_time_index,
);
criterion_main!(benches);
