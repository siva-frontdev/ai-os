//! Criterion benchmarks for the memory-cache crate.

use std::time::Duration;

use criterion::{BatchSize, Criterion, black_box, criterion_group, criterion_main};
use memory_cache::cache::{LruMemoryCache, MemoryCache};
use memory_core::MemoryObject;

fn make_obj(data: Vec<u8>) -> MemoryObject {
    MemoryObject::new("bench".into(), data, "text/plain".into())
}

fn rt() -> tokio::runtime::Runtime {
    tokio::runtime::Runtime::new().expect("failed to build runtime")
}

fn bench_get_set(c: &mut Criterion) {
    let rt = rt();

    let mut group = c.benchmark_group("get_set");
    group.sample_size(100);

    group.bench_function("set_1k", |b| {
        let cache = LruMemoryCache::new(10_000, 1_000_000_000);
        b.to_async(&rt).iter_custom(|iters| {
            let cache = cache.clone();
            async move {
                for i in 0..iters {
                    let key = format!("key{i}");
                    let obj = make_obj(vec![0u8; 1024]);
                    cache
                        .set(black_box(key), black_box(obj), None)
                        .await
                        .unwrap();
                }
            }
        });
    });

    group.bench_function("get_existing", |b| {
        let cache = LruMemoryCache::new(10_000, 1_000_000_000);
        let rt = rt();
        // Pre-populate
        for i in 0..5_000 {
            let key = format!("key{i}");
            let obj = make_obj(vec![0u8; 128]);
            rt.block_on(cache.set(key, obj, None)).unwrap();
        }

        b.to_async(&rt).iter_custom(|iters| {
            let cache = cache.clone();
            async move {
                for i in 0..iters {
                    let key = format!("key{}", i % 5_000);
                    let _ = cache.get(black_box(&key)).await;
                }
            }
        });
    });

    group.finish();
}

fn bench_lru_eviction(c: &mut Criterion) {
    let rt = rt();

    let mut group = c.benchmark_group("lru_eviction");
    group.sample_size(50);

    group.bench_function("evict_oldest", |b| {
        b.to_async(&rt).iter_custom(|iters| {
            async move {
                for _ in 0..iters {
                    let cache = LruMemoryCache::new(500, 1_000_000_000);
                    // Fill to capacity
                    for i in 0..500 {
                        let key = format!("key{i}");
                        let obj = make_obj(vec![0u8; 128]);
                        cache.set(key, obj, None).await.unwrap();
                    }
                    // Trigger eviction by adding one more
                    let obj = make_obj(vec![0u8; 128]);
                    cache.set("evict_me".into(), obj, None).await.unwrap();
                }
            }
        });
    });

    group.finish();
}

fn bench_concurrent_access(c: &mut Criterion) {
    let rt = rt();

    let mut group = c.benchmark_group("concurrent");
    group.sample_size(50);

    group.bench_function("10_tasks_get_set", |b| {
        b.to_async(&rt).iter_custom(|iters| {
            async move {
                for _ in 0..iters {
                    let cache = std::sync::Arc::new(LruMemoryCache::new(10_000, 1_000_000_000));
                    // Pre-populate
                    for i in 0..1_000 {
                        let key = format!("key{i}");
                        let obj = make_obj(vec![0u8; 64]);
                        cache.set(key, obj, None).await.unwrap();
                    }

                    let mut handles = Vec::new();
                    for t in 0..10 {
                        let c = cache.clone();
                        handles.push(tokio::spawn(async move {
                            for i in 0..100 {
                                let key = format!("key{}", (t * 100 + i) % 1_000);
                                let _ = c.get(&key).await;
                            }
                        }));
                    }
                    for h in handles {
                        h.await.unwrap();
                    }
                }
            }
        });
    });

    group.finish();
}

fn bench_mixed_workload(c: &mut Criterion) {
    let rt = rt();

    let mut group = c.benchmark_group("mixed_workload");
    group.sample_size(50);

    group.bench_function("50_percent_hit_miss", |b| {
        b.to_async(&rt).iter_custom(|iters| {
            async move {
                for _ in 0..iters {
                    let cache = LruMemoryCache::new(5_000, 1_000_000_000);
                    // Populate half
                    for i in 0..2_500 {
                        let key = format!("key{i}");
                        let obj = make_obj(vec![0u8; 64]);
                        cache.set(key, obj, None).await.unwrap();
                    }

                    for i in 0..5_000 {
                        let key = format!("key{}", i); // 0..2500 exist, 2500..5000 don't
                        let _ = cache.get(&key).await;
                        // Every 10th operation, insert a fresh key
                        if i % 10 == 0 {
                            let obj = make_obj(vec![0u8; 64]);
                            cache.set(format!("fresh{i}"), obj, None).await.unwrap();
                        }
                    }
                }
            }
        });
    });

    group.finish();
}

criterion_group! {
    name = cache_bench;
    config = Criterion::default().measurement_time(Duration::from_secs(10));
    targets = bench_get_set, bench_lru_eviction, bench_concurrent_access, bench_mixed_workload
}
criterion_main!(cache_bench);
