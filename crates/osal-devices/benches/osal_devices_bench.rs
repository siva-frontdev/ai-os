use criterion::{criterion_group, criterion_main, Criterion};
use osal_capabilities::CapabilityContext;
use osal_core::DeviceManager;
use osal_devices::DefaultDeviceManager;

fn dummy_ctx() -> CapabilityContext {
    CapabilityContext::new("bench")
}

fn bench_default_enumerate(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mgr = DefaultDeviceManager;
    let ctx = dummy_ctx();

    c.bench_function("default_enumerate", |b| {
        b.to_async(&rt).iter(|| mgr.enumerate(&ctx));
    });
}

fn bench_default_access(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mgr = DefaultDeviceManager;
    let ctx = dummy_ctx();

    c.bench_function("default_access", |b| {
        b.to_async(&rt).iter(|| mgr.access(&ctx, "/dev/null"));
    });
}

fn bench_default_events(c: &mut Criterion) {
    let mgr = DefaultDeviceManager;

    c.bench_function("default_events", |b| {
        b.iter(|| {
            let _rx = mgr.events();
        });
    });
}

criterion_group!(benches, bench_default_enumerate, bench_default_access, bench_default_events);
criterion_main!(benches);
