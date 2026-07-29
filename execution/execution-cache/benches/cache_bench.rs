use criterion::{criterion_group, criterion_main, Criterion};

fn bench_cache_new(c: &mut Criterion) {
    c.bench_function("cache_new", |b| {
        b.iter(|| execution_cache::InMemoryCapabilityCache::new())
    });
}

criterion_group!(benches, bench_cache_new);
criterion_main!(benches);
