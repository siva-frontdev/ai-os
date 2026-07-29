use criterion::{criterion_group, criterion_main, Criterion};

fn bench_discovery_new(c: &mut Criterion) {
    c.bench_function("discovery_new", |b| {
        b.iter(|| execution_discovery::DefaultLocalDiscoverer::new())
    });
}

criterion_group!(benches, bench_discovery_new);
criterion_main!(benches);
