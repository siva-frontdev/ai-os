use criterion::{black_box, criterion_group, criterion_main, Criterion};

use osal_monitoring::{
    CpuCore, CpuSnapshot, DiskSnapshot, MemorySnapshot, NetworkIoSnapshot,
    OsInfo, ResourceThreshold, ResourceType, SystemLoad, TemperatureSnapshot,
    ThresholdOperator, DefaultSystemMonitor,
};
use osal_core::SystemMonitor;
use osal_capabilities::CapabilityContext;

fn dummy_ctx() -> CapabilityContext {
    CapabilityContext::new("bench")
}

fn bench_cpu_snapshot_serialize(c: &mut Criterion) {
    let snap = CpuSnapshot {
        usage_percent: 35.0,
        user_percent: 20.0,
        system_percent: 10.0,
        iowait_percent: 3.0,
        steal_percent: 2.0,
        cores: (0..8)
            .map(|i| CpuCore {
                index: i,
                usage_percent: 30.0 + i as f64,
                frequency_mhz: 2400,
            })
            .collect(),
        context_switches: 1_000_000,
        processes: 200,
    };
    c.bench_function("cpu_snapshot_serialize", |b| {
        b.iter(|| serde_json::to_string(black_box(&snap)))
    });
}

fn bench_cpu_snapshot_deserialize(c: &mut Criterion) {
    let snap = CpuSnapshot {
        usage_percent: 35.0,
        user_percent: 20.0,
        system_percent: 10.0,
        iowait_percent: 3.0,
        steal_percent: 2.0,
        cores: (0..8)
            .map(|i| CpuCore {
                index: i,
                usage_percent: 30.0 + i as f64,
                frequency_mhz: 2400,
            })
            .collect(),
        context_switches: 1_000_000,
        processes: 200,
    };
    let json = serde_json::to_string(&snap).unwrap();
    c.bench_function("cpu_snapshot_deserialize", |b| {
        b.iter(|| serde_json::from_str::<CpuSnapshot>(black_box(&json)))
    });
}

fn bench_memory_snapshot_serialize(c: &mut Criterion) {
    let snap = MemorySnapshot {
        total_bytes: 16_000_000_000,
        available_bytes: 8_000_000_000,
        used_bytes: 7_000_000_000,
        cached_bytes: 2_000_000_000,
        buffers_bytes: 500_000_000,
        swap_total: 2_000_000_000,
        swap_used: 100_000_000,
        swap_percent: 5.0,
    };
    c.bench_function("memory_snapshot_serialize", |b| {
        b.iter(|| serde_json::to_string(black_box(&snap)))
    });
}

fn bench_disk_snapshot_serialize(c: &mut Criterion) {
    let snap = DiskSnapshot {
        mount_point: "/".into(),
        device: "/dev/sda1".into(),
        total_bytes: 500_000_000_000,
        used_bytes: 200_000_000_000,
        available_bytes: 300_000_000_000,
        usage_percent: 40.0,
        read_bytes_per_sec: 100_000,
        write_bytes_per_sec: 50_000,
        iops_read: 100,
        iops_write: 50,
    };
    c.bench_function("disk_snapshot_serialize", |b| {
        b.iter(|| serde_json::to_string(black_box(&snap)))
    });
}

fn bench_network_snapshot_serialize(c: &mut Criterion) {
    let snap = NetworkIoSnapshot {
        interface: "eth0".into(),
        rx_bytes: 10_000_000,
        tx_bytes: 5_000_000,
        rx_packets: 100_000,
        tx_packets: 50_000,
        rx_errors: 0,
        tx_errors: 1,
        rx_dropped: 2,
        tx_dropped: 0,
    };
    c.bench_function("network_snapshot_serialize", |b| {
        b.iter(|| serde_json::to_string(black_box(&snap)))
    });
}

fn bench_temperature_snapshot_serialize(c: &mut Criterion) {
    let snap = TemperatureSnapshot {
        sensor: "coretemp-isa-0000".into(),
        label: "Package id 0".into(),
        current_celsius: 65.0,
        high_celsius: Some(85.0),
        critical_celsius: Some(100.0),
    };
    c.bench_function("temperature_snapshot_serialize", |b| {
        b.iter(|| serde_json::to_string(black_box(&snap)))
    });
}

fn bench_system_load_serialize(c: &mut Criterion) {
    let load = SystemLoad {
        load_1m: 1.5,
        load_5m: 2.0,
        load_15m: 1.8,
        running_processes: 3,
        total_processes: 250,
    };
    c.bench_function("system_load_serialize", |b| {
        b.iter(|| serde_json::to_string(black_box(&load)))
    });
}

fn bench_os_info_serialize(c: &mut Criterion) {
    let info = OsInfo {
        hostname: "archbox".into(),
        os_name: "Arch Linux".into(),
        os_version: "rolling".into(),
        kernel_version: "6.5.7-arch1-1".into(),
        architecture: "x86_64".into(),
    };
    c.bench_function("os_info_serialize", |b| {
        b.iter(|| serde_json::to_string(black_box(&info)))
    });
}

fn bench_resource_threshold_serialize(c: &mut Criterion) {
    let t = ResourceThreshold {
        resource: ResourceType::Cpu,
        metric: "usage_percent".into(),
        operator: ThresholdOperator::GreaterThan,
        value: 90.0,
        cooldown_seconds: 60,
    };
    c.bench_function("resource_threshold_serialize", |b| {
        b.iter(|| serde_json::to_string(black_box(&t)))
    });
}

fn bench_default_cpu_usage(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mon = DefaultSystemMonitor::new();
    let ctx = dummy_ctx();
    c.bench_function("default_cpu_usage", |b| {
        b.to_async(&rt).iter(|| mon.cpu_usage(black_box(&ctx)))
    });
}

fn bench_default_memory_info(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mon = DefaultSystemMonitor::new();
    let ctx = dummy_ctx();
    c.bench_function("default_memory_info", |b| {
        b.to_async(&rt).iter(|| mon.memory_info(black_box(&ctx)))
    });
}

fn bench_default_events(c: &mut Criterion) {
    let mon = DefaultSystemMonitor::new();
    c.bench_function("default_events", |b| {
        b.iter(|| {
            let _rx = mon.events();
        })
    });
}

criterion_group!(
    benches,
    bench_cpu_snapshot_serialize,
    bench_cpu_snapshot_deserialize,
    bench_memory_snapshot_serialize,
    bench_disk_snapshot_serialize,
    bench_network_snapshot_serialize,
    bench_temperature_snapshot_serialize,
    bench_system_load_serialize,
    bench_os_info_serialize,
    bench_resource_threshold_serialize,
    bench_default_cpu_usage,
    bench_default_memory_info,
    bench_default_events,
);
criterion_main!(benches);
