use criterion::{criterion_group, criterion_main, Criterion};

fn bench_adapter_new(c: &mut Criterion) {
    c.bench_function("adapter_new", |b| {
        b.iter(|| execution_adapter::DefaultAdapterGenerator::new())
    });
}

criterion_group!(benches, bench_adapter_new);
criterion_main!(benches);
