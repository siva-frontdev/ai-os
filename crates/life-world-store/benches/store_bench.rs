//! Criterion benchmarks for the World Store (RFC-0008 §Benchmarks).

use std::collections::HashMap;
use std::time::Duration;

use criterion::{BatchSize, BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use life_world_store::{
    CreateEntity, SearchQuery, SqliteWorldModelStore, WorldStore, WorldStoreConfig, WriteContext,
};
use memory_core::wm::Value;

fn temp_store() -> (tempfile::TempDir, SqliteWorldModelStore) {
    let dir = tempfile::tempdir().expect("tempdir");
    let config = WorldStoreConfig {
        db_path: dir.path().join("bench.db"),
        documents_dir: dir.path().join("docs"),
        wal: false,
        enable_fts: true,
        ..WorldStoreConfig::default()
    };
    let store = SqliteWorldModelStore::new(config);
    (dir, store)
}

fn create_entity(name: &str) -> CreateEntity {
    let mut properties = HashMap::new();
    properties.insert(
        "email".to_string(),
        Value::String(format!("{name}@example.com")),
    );
    CreateEntity {
        entity_type: "person".to_string(),
        name: name.to_string(),
        properties,
        importance: 0.5,
        confidence: 0.8,
        metadata: HashMap::new(),
        ctx: WriteContext::ai(),
    }
}

fn bench_insert(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let (_dir, store) = temp_store();
    rt.block_on(store.init()).expect("init");

    let mut group = c.benchmark_group("world_store");
    group.throughput(criterion::Throughput::Elements(1));
    group.bench_function("insert_entity", |b| {
        b.iter_batched(
            || create_entity(&format!("user-{}", black_box(rand_id()))),
            |req| rt.block_on(store.create_entity(req)).expect("create"),
            BatchSize::SmallInput,
        )
    });
    group.finish();
}

fn bench_search(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let (_dir, store) = temp_store();
    rt.block_on(store.init()).expect("init");
    for i in 0..200 {
        rt.block_on(store.create_entity(create_entity(&format!("Person {}", i))))
            .expect("seed");
    }

    let mut group = c.benchmark_group("world_store");
    let query = SearchQuery {
        text: Some("Person".to_string()),
        limit: 10,
        ..Default::default()
    };
    group.bench_function(BenchmarkId::new("search_entities", "fts"), |b| {
        b.iter(|| {
            let result = rt.block_on(store.search_entities(black_box(&query)));
            black_box(result.expect("search").len())
        })
    });
    group.finish();
}

fn rand_id() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .warm_up_time(Duration::from_millis(300))
        .measurement_time(Duration::from_secs(1));
    targets = bench_insert, bench_search
}
criterion_main!(benches);
