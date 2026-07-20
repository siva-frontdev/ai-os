//! Criterion benchmarks for the memory-cache crate.

use std::time::Duration;

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use memory_cache::cache::{LruMemoryCache, MemoryCache};
use memory_core::MemoryObject;

fn make_obj(data: Vec<u8>) -> MemoryObject {
    MemoryObject::builder()
        .content_type("text/plain")
        .content(data)
        .build()
}

fn bench_get_set(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().expect("runtime");

    let mut group = c.benchmark_group("get_set");
    group.sample_size(100);

    group.bench_function("set_1k", |b| {
        b.to_async(&rt).iter_custom(|iters| async move {
            let cache = LruMemoryCache::new(10_000, 1_000_000_000);
            let start = std::time::Instant::now();
            for i in 0..iters {
                let key = format!("key{i}");
                let obj = make_obj(vec![0u8; 1024]);
                cache.set(black_box(key), black_box(obj), None).await.unwrap();
            }
            start.elapsed()
        });
    });

    group.bench_function("get_existing", |b| {
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        b.to_async(&rt).iter_custom(|iters| async move {
            let cache = LruMemoryCache::new(10_000, 1_000_000_000);
            for i in 0..5_000 {
                let key = format!("key{i}");
                let obj = make_obj(vec![0u8; 128]);
                cache.set(key, obj, None).await.unwrap();
            }
            let start = std::time::Instant::now();
            for i in 0..iters {
                let key = format!("key{}", i % 5_000);
                let _ = cache.get(black_box(&key)).await;
            }
            start.elapsed()
        });
    });

    group.finish();
}

fn bench_lru_eviction(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().expect("runtime");

    let mut group = c.benchmark_group("lru_eviction");
    group.sample_size(50);

    group.bench_function("evict_oldest", |b| {
        b.to_async(&rt).iter_custom(|iters| async move {
            let start = std::time::Instant::now();
            for _ in 0..iters {
                let cache = LruMemoryCache::new(500, 1_000_000_000);
                for i in 0..500 {
                    let key = format!("key{i}");
                    let obj = make_obj(vec![0u8; 128]);
                    cache.set(key, obj, None).await.unwrap();
                }
                let obj = make_obj(vec![0u8; 128]);
                cache.set("evict_me".into(), obj, None).await.unwrap();
            }
            start.elapsed()
        });
    });

    group.finish();
}

criterion_group! {
    name = cache_bench;
    config = Criterion::default().measurement_time(Duration::from_secs(10));
    targets = bench_get_set, bench_lru_eviction
}
criterion_main!(cache_bench);
