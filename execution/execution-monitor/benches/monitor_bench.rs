use criterion::{criterion_group, criterion_main, Criterion};

use execution_core::traits::ExecutionMonitor;
use execution_core::types::*;
use execution_monitor::DefaultExecutionMonitor;

fn bench_watch_creation(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().expect("failed to create runtime");

    c.bench_function("watch_creation", |b| {
        b.to_async(&rt).iter(|| async {
            let monitor = DefaultExecutionMonitor::without_event_bus();
            let id = ExecutionId::new();
            let budget = ExecutionBudget::new(60_000);
            let (cancel_tx, _cancel_rx) = tokio::sync::oneshot::channel();
            monitor.start_watch(id, budget, cancel_tx).await;
        })
    });
}

fn bench_metric_reporting(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().expect("failed to create runtime");

    c.bench_function("metric_reporting", |b| {
        b.to_async(&rt).iter(|| async {
            let monitor = DefaultExecutionMonitor::without_event_bus();
            let id = ExecutionId::new();
            let budget = ExecutionBudget::new(60_000);
            let (cancel_tx, _cancel_rx) = tokio::sync::oneshot::channel();
            monitor.start_watch(id, budget, cancel_tx).await;

            let metrics = ExecutionMetrics {
                wall_clock_ms: 42,
                cpu_ms: 10,
                peak_memory_bytes: 4096,
                io_read_bytes: 128,
                io_write_bytes: 64,
            };
            monitor.report_heartbeat(&id, metrics).await;
            let _ = monitor.current_metrics(&id).await;
        })
    });
}

criterion_group!(benches, bench_watch_creation, bench_metric_reporting);
criterion_main!(benches);
