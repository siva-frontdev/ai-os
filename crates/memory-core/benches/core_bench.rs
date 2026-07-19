use criterion::{black_box, criterion_group, criterion_main, Criterion};

use memory_core::*;

fn bench_memory_id_creation(c: &mut Criterion) {
    c.bench_function("MemoryId::new", |b| {
        b.iter(|| MemoryId::new());
    });
}

fn bench_timestamp_now(c: &mut Criterion) {
    c.bench_function("Timestamp::now", |b| {
        b.iter(|| Timestamp::now());
    });
}

fn bench_memory_object_build(c: &mut Criterion) {
    c.bench_function("MemoryObject::build (default)", |b| {
        b.iter(|| {
            MemoryObject::builder()
                .content_type(black_box("text/plain"))
                .content(black_box(b"hello world".to_vec()))
                .build()
        });
    });
}

fn bench_memory_object_build_complex(c: &mut Criterion) {
    let tags: Vec<String> = (0..10).map(|i| format!("tag-{}", i)).collect();
    let relationships: Vec<Relationship> = (0..10)
        .map(|_| Relationship::reference(MemoryId::new()))
        .collect();

    c.bench_function("MemoryObject::build (complex)", |b| {
        b.iter(|| {
            MemoryObject::builder()
                .content_type(black_box("application/json"))
                .content(black_box(br#"{"data":"sample"}"#.to_vec()))
                .importance(MemoryImportance::new(0.9).unwrap())
                .priority(MemoryPriority::HIGH)
                .tags(black_box(tags.clone()))
                .relationships(black_box(relationships.clone()))
                .embedding(Some(black_box(vec![0.1f32; 384])))
                .tag("extra")
                .build()
        });
    });
}

fn bench_checksum_compute(c: &mut Criterion) {
    let data = vec![0u8; 1024];
    c.bench_function("Checksum::compute (1KB)", |b| {
        b.iter(|| Checksum::compute(black_box(&data)));
    });
}

fn bench_serialization(c: &mut Criterion) {
    let obj = MemoryObject::builder()
        .content_type("text/plain")
        .content(vec![0u8; 1024])
        .tag("benchmark")
        .build();

    c.bench_function("MemoryObject::to_json_bytes", |b| {
        b.iter(|| obj.to_json_bytes().unwrap());
    });

    let json = obj.to_json_bytes().unwrap();
    c.bench_function("MemoryObject::from_json_bytes", |b| {
        b.iter(|| MemoryObject::from_json_bytes(black_box(&json)).unwrap());
    });
}

fn bench_validation(c: &mut Criterion) {
    let obj = MemoryObject::builder()
        .content_type("text/plain")
        .content(vec![0u8; 1024])
        .build();

    c.bench_function("MemoryObject::validate", |b| {
        b.iter(|| obj.validate().unwrap());
    });
}

criterion_group!(
    name = benches;
    config = Criterion::default();
    targets =
        bench_memory_id_creation,
        bench_timestamp_now,
        bench_memory_object_build,
        bench_memory_object_build_complex,
        bench_checksum_compute,
        bench_serialization,
        bench_validation,
);
criterion_main!(benches);
