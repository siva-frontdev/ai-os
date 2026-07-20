use criterion::{black_box, criterion_group, criterion_main, Criterion};
use perception_core::*;

fn bench_observation_creation(c: &mut Criterion) {
    let source = ObservationSource {
        observer_id: "bench".into(),
        observer_kind: ObserverKind::FileSystem,
        instance_id: "inst-1".into(),
        hostname: "host".into(),
    };

    c.bench_function("observation_create", |b| {
        b.iter(|| {
            let obs = Observation::new(
                source.clone(),
                Modality::FileSystem,
                ObservationPriority::Normal,
                ObservationPayload::Text {
                    content: "benchmark data".into(),
                    encoding: "utf-8".into(),
                },
            );
            black_box(obs);
        })
    });
}

fn bench_observation_id_generation(c: &mut Criterion) {
    c.bench_function("observation_id_generate", |b| {
        b.iter(|| {
            let id = ObservationId::new();
            black_box(id);
        })
    });
}

fn bench_confidence_creation(c: &mut Criterion) {
    c.bench_function("confidence_create", |b| {
        b.iter(|| {
            let c = Confidence::new(black_box(0.75));
            black_box(c);
        })
    });
}

criterion_group!(
    benches,
    bench_observation_creation,
    bench_observation_id_generation,
    bench_confidence_creation
);
criterion_main!(benches);
