use criterion::{Criterion, criterion_group, criterion_main};

use memory_core::{MemoryId, RelationType, Timestamp};
use memory_index::*;

fn bench_default_metadata_index(c: &mut Criterion) {
    let idx = DefaultMetadataIndex;
    let id = MemoryId::new();

    c.bench_function("DefaultMetadataIndex::index_metadata", |b| {
        b.iter(|| {
            let _ = idx.index_metadata(&id, "key", "value");
        });
    });
}

fn bench_default_tag_index(c: &mut Criterion) {
    let idx = DefaultTagIndex;
    let id = MemoryId::new();
    let tags = vec!["a".into(), "b".into(), "c".into()];

    c.bench_function("DefaultTagIndex::index_tags", |b| {
        b.iter(|| {
            let _ = idx.index_tags(&id, &tags);
        });
    });
}

fn bench_default_relationship_index(c: &mut Criterion) {
    let idx = DefaultRelationshipIndex;
    let a = MemoryId::new();
    let b = MemoryId::new();

    c.bench_function("DefaultRelationshipIndex::index_relationship", |b| {
        b.iter(|| {
            let _ = idx.index_relationship(&a, &b, &RelationType::References);
        });
    });
}

fn bench_default_time_index(c: &mut Criterion) {
    let idx = DefaultTimeIndex;
    let id = MemoryId::new();
    let now = Timestamp::now();

    c.bench_function("DefaultTimeIndex::index_time", |b| {
        b.iter(|| {
            let _ = idx.index_time(&id, now);
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
