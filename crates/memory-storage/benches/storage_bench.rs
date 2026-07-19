use criterion::{Criterion, black_box, criterion_group, criterion_main};

use memory_core::{MemoryObject, MemoryPriority, MemoryTier, MemoryType, QueryFilter, Timestamp};
use memory_storage::{InMemoryStore, MemoryStore};

fn create_test_object(size: usize, tier: MemoryTier, memory_type: MemoryType) -> MemoryObject {
    MemoryObject::builder()
        .content_type("application/octet-stream")
        .content(vec![0u8; size])
        .tier(tier)
        .memory_type(memory_type)
        .build()
}

fn bench_in_memory_insert(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("InMemoryStore::insert (1KB object)", |b| {
        b.iter(|| {
            let store = InMemoryStore::new();
            let obj = create_test_object(1024, MemoryTier::Working, MemoryType::Working);
            rt.block_on(async { store.insert(black_box(obj)).await.unwrap() });
        });
    });
}

fn bench_in_memory_batch_insert(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("InMemoryStore::insert_batch (100 objects)", |b| {
        b.iter(|| {
            let store = InMemoryStore::new();
            let objects: Vec<MemoryObject> = (0..100)
                .map(|i| {
                    MemoryObject::builder()
                        .content_type("text")
                        .content(vec![i as u8])
                        .build()
                })
                .collect();
            rt.block_on(async { store.insert_batch(black_box(&objects)).await.unwrap() });
        });
    });
}

fn bench_in_memory_get(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let store = InMemoryStore::new();
    let obj = create_test_object(1024, MemoryTier::Working, MemoryType::Working);
    rt.block_on(async { store.insert(obj.clone()).await.unwrap() });
    let id = obj.id;

    c.bench_function("InMemoryStore::get (hit)", |b| {
        b.iter(|| {
            rt.block_on(async { store.get(black_box(&id)).await.unwrap() });
        });
    });
}

fn bench_in_memory_query(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let store = InMemoryStore::new();

    for i in 0..1000 {
        let obj = MemoryObject::builder()
            .content_type("text")
            .content(vec![i as u8])
            .tier(if i % 2 == 0 {
                MemoryTier::Working
            } else {
                MemoryTier::Episodic
            })
            .build();
        rt.block_on(async { store.insert(obj).await.unwrap() });
    }

    let filter = QueryFilter {
        tier: Some(MemoryTier::Working),
        limit: 50,
        ..Default::default()
    };

    c.bench_function("InMemoryStore::query (filtered, 1000 objects)", |b| {
        b.iter(|| {
            rt.block_on(async { store.query(black_box(&filter)).await.unwrap() });
        });
    });
}

fn bench_in_memory_stats(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let store = InMemoryStore::new();

    for i in 0..100 {
        let obj = create_test_object(512, MemoryTier::Working, MemoryType::Working);
        rt.block_on(async { store.insert(obj).await.unwrap() });
    }

    c.bench_function("InMemoryStore::stats (100 objects)", |b| {
        b.iter(|| {
            rt.block_on(async { store.stats().await.unwrap() });
        });
    });
}

criterion_group!(
    name = benches;
    config = Criterion::default();
    targets =
        bench_in_memory_insert,
        bench_in_memory_batch_insert,
        bench_in_memory_get,
        bench_in_memory_query,
        bench_in_memory_stats,
);
criterion_main!(benches);
