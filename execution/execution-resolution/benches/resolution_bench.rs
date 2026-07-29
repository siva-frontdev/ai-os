use criterion::{criterion_group, criterion_main, Criterion};

fn bench_resolution_new(c: &mut Criterion) {
    c.bench_function("resolution_new", |b| {
        b.iter(|| {
            let registry = std::sync::Arc::new(execution_registry::InMemoryToolRegistry::new());
            let sandbox = std::sync::Arc::new(execution_sandbox::DefaultSandboxEnforcer::new());
            execution_resolution::DefaultResolutionPipeline::new(registry, sandbox, None)
        })
    });
}

criterion_group!(benches, bench_resolution_new);
criterion_main!(benches);
