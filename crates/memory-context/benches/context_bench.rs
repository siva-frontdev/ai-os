use criterion::{Criterion, criterion_group, criterion_main};
use memory_context::{ContextManager, TreeContextManager};

fn bench_create_context(c: &mut Criterion) {
    c.bench_function("context_create", |b| {
        b.iter(|| {
            let mgr = TreeContextManager::new();
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async { mgr.create_context(None).await.unwrap() });
        });
    });
}

fn bench_value_set(c: &mut Criterion) {
    c.bench_function("context_set_value", |b| {
        b.iter(|| {
            let mgr = TreeContextManager::new();
            let rt = tokio::runtime::Runtime::new().unwrap();
            let id = rt.block_on(async { mgr.create_context(None).await.unwrap() });
            rt.block_on(async move {
                mgr.set_value(&id, "bench_key", b"bench_value".to_vec())
                    .await
                    .unwrap();
            });
        });
    });
}

fn bench_snapshot(c: &mut Criterion) {
    c.bench_function("context_snapshot", |b| {
        b.iter(|| {
            let mgr = TreeContextManager::new();
            let rt = tokio::runtime::Runtime::new().unwrap();
            let id = rt.block_on(async { mgr.create_context(None).await.unwrap() });
            rt.block_on(async move {
                mgr.snapshot(&id).await.unwrap();
            });
        });
    });
}

criterion_group!(
    benches,
    bench_create_context,
    bench_value_set,
    bench_snapshot
);
criterion_main!(benches);
