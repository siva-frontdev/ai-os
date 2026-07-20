use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_execution_id_new(c: &mut Criterion) {
    c.bench_function("execution_id_new", |b| {
        b.iter(|| execution_core::ExecutionId::new())
    });
}

fn bench_execution_id_parse(c: &mut Criterion) {
    let id = execution_core::ExecutionId::new().to_string();
    c.bench_function("execution_id_parse", |b| {
        b.iter(|| id.parse::<execution_core::ExecutionId>())
    });
}

fn bench_retry_backoff(c: &mut Criterion) {
    let policy = execution_core::RetryPolicy::default();
    c.bench_function("retry_backoff", |b| {
        b.iter(|| {
            for attempt in 0..10 {
                black_box(policy.compute_backoff(attempt));
            }
        })
    });
}

criterion_group!(
    benches,
    bench_execution_id_new,
    bench_execution_id_parse,
    bench_retry_backoff
);
criterion_main!(benches);
