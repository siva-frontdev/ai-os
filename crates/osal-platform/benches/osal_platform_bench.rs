use criterion::{black_box, criterion_group, criterion_main, Criterion};
use osal_platform::{HardwareInformation, KernelInformation, OsInformation};

fn bench_os_information_serde(c: &mut Criterion) {
    let info = OsInformation {
        name: "Arch Linux".into(),
        version: "rolling".into(),
        version_id: "2024.01".into(),
        pretty_name: "Arch Linux".into(),
        id_like: None,
        home_url: Some("https://archlinux.org".into()),
        support_url: None,
    };

    c.bench_function("os_information_serialize", |b| {
        b.iter(|| serde_json::to_string(black_box(&info)))
    });

    let json = serde_json::to_string(&info).unwrap();
    c.bench_function("os_information_deserialize", |b| {
        b.iter(|| {
            let _: OsInformation = serde_json::from_str(black_box(&json)).unwrap();
        })
    });
}

fn bench_kernel_information_serde(c: &mut Criterion) {
    let info = KernelInformation {
        release: "6.6.1-arch1".into(),
        version: "#1 SMP PREEMPT_DYNAMIC Mon Jan 15 00:00:00 UTC 2024".into(),
        architecture: "x86_64".into(),
        build_date: Some("2024-01-15".into()),
    };

    c.bench_function("kernel_information_serialize", |b| {
        b.iter(|| serde_json::to_string(black_box(&info)))
    });

    let json = serde_json::to_string(&info).unwrap();
    c.bench_function("kernel_information_deserialize", |b| {
        b.iter(|| {
            let _: KernelInformation = serde_json::from_str(black_box(&json)).unwrap();
        })
    });
}

fn bench_hardware_information_serde(c: &mut Criterion) {
    let info = HardwareInformation {
        model: Some("ThinkPad X1 Carbon Gen 11".into()),
        serial: Some("ABC123456789".into()),
        manufacturer: Some("Lenovo".into()),
        total_memory_bytes: 17179869184,
        processor_count: 8,
        processor_model: Some("Intel Core i7-1365U".into()),
        processor_frequency_mhz: Some(1800),
        virtualization: Some("KVM".into()),
    };

    c.bench_function("hardware_information_serialize", |b| {
        b.iter(|| serde_json::to_string(black_box(&info)))
    });

    let json = serde_json::to_string(&info).unwrap();
    c.bench_function("hardware_information_deserialize", |b| {
        b.iter(|| {
            let _: HardwareInformation = serde_json::from_str(black_box(&json)).unwrap();
        })
    });
}

criterion_group!(
    benches,
    bench_os_information_serde,
    bench_kernel_information_serde,
    bench_hardware_information_serde,
);
criterion_main!(benches);
