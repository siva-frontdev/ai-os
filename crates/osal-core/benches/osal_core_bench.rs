use criterion::{black_box, criterion_group, criterion_main, Criterion};
use osal_capabilities::CapabilityContext;
use osal_core::mock::{MockFileSystem, MockProcessManager, MockNetworkManager, MockSystemMonitor};

fn create_ctx() -> CapabilityContext {
    CapabilityContext::new("bench")
}

fn bench_mock_filesystem_read(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let fs = MockFileSystem::new();
    let ctx = create_ctx();

    c.bench_function("mock_filesystem_read", |b| {
        b.to_async(&rt).iter(|| fs.read(black_box(&ctx), black_box("/tmp/bench_file.txt")))
    });
}

fn bench_mock_process_spawn(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let pm = MockProcessManager::new();
    let ctx = create_ctx();

    c.bench_function("mock_process_spawn", |b| {
        b.to_async(&rt).iter(|| pm.spawn(black_box(&ctx), black_box("ls"), black_box(&["-la"])))
    });
}

fn bench_mock_network_dns_lookup(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let net = MockNetworkManager::new();
    let ctx = create_ctx();

    c.bench_function("mock_network_dns_lookup", |b| {
        b.to_async(&rt).iter(|| net.dns_lookup(black_box(&ctx), black_box("localhost")))
    });
}

fn bench_mock_monitor_cpu_usage(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mon = MockSystemMonitor::new();
    let ctx = create_ctx();

    c.bench_function("mock_monitor_cpu_usage", |b| {
        b.to_async(&rt).iter(|| mon.cpu_usage(black_box(&ctx)))
    });
}

criterion_group!(
    benches,
    bench_mock_filesystem_read,
    bench_mock_process_spawn,
    bench_mock_network_dns_lookup,
    bench_mock_monitor_cpu_usage,
);
criterion_main!(benches);
