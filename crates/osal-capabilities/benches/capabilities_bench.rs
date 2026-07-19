use criterion::{black_box, criterion_group, criterion_main, Criterion};
use osal_capabilities::{Capability, CapabilitySet};

fn bench_capability_check_hit(c: &mut Criterion) {
    let mut set = CapabilitySet::new();
    set.grant(Capability::ProcessSpawn);

    c.bench_function("capability_check_hit", |b| {
        b.iter(|| set.check(black_box(&Capability::ProcessSpawn)))
    });
}

fn bench_capability_check_miss(c: &mut Criterion) {
    let set = CapabilitySet::new();

    c.bench_function("capability_check_miss", |b| {
        b.iter(|| set.check(black_box(&Capability::ProcessSpawn)))
    });
}

fn bench_capability_check_admin(c: &mut Criterion) {
    let mut set = CapabilitySet::new();
    set.grant(Capability::Admin);

    c.bench_function("capability_check_admin", |b| {
        b.iter(|| set.check(black_box(&Capability::ProcessSpawn)))
    });
}

fn bench_capability_check_path(c: &mut Criterion) {
    let mut set = CapabilitySet::new();
    set.grant(Capability::FileRead("*".into()));

    c.bench_function("capability_check_path", |b| {
        b.iter(|| set.check_path(black_box(Capability::FileRead), black_box("/etc/passwd")))
    });
}

fn bench_capability_serde(c: &mut Criterion) {
    let mut set = CapabilitySet::new();
    set.grant(Capability::ProcessSpawn);
    set.grant(Capability::Admin);
    set.grant(Capability::FileRead("/tmp/foo.txt".into()));

    c.bench_function("capability_serde_roundtrip", |b| {
        b.iter(|| {
            let json = serde_json::to_string(black_box(&set)).unwrap();
            let _: CapabilitySet = serde_json::from_str(&json).unwrap();
        })
    });
}

criterion_group!(
    benches,
    bench_capability_check_hit,
    bench_capability_check_miss,
    bench_capability_check_admin,
    bench_capability_check_path,
    bench_capability_serde,
);
criterion_main!(benches);
