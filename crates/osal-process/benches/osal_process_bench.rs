use std::collections::HashMap;
use std::time::Duration;

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use osal_capabilities::{CapabilityContext, CapabilitySet};
use osal_core::{Pid, Uid};
use osal_process::{DefaultProcessManager, ProcessConfig, ProcessState, ProcessStatus};

fn create_process_config() -> ProcessConfig {
    let mut env = HashMap::new();
    env.insert("PATH".into(), "/usr/bin:/bin".into());
    ProcessConfig {
        command: "sleep".into(),
        args: vec!["10".into()],
        env,
        working_dir: Some("/tmp".into()),
        uid: Some(Uid(1000)),
        gid: Some(Uid(1000).0.into()),
        timeout: Some(Duration::from_secs(30)),
        capabilities: CapabilitySet::new(),
        stdin: None,
        stdout: None,
        stderr: None,
    }
}

fn create_process_status() -> ProcessStatus {
    ProcessStatus {
        pid: Pid(1234),
        state: ProcessState::Running,
        exit_status: None,
        cpu_usage: 1.5,
        memory_usage: 4_194_304,
        user: Uid(1000),
        running_time: Duration::from_secs(120),
    }
}

fn bench_process_config_serialize(c: &mut Criterion) {
    let config = create_process_config();

    c.bench_function("process_config_serialize", |b| {
        b.iter(|| serde_json::to_string(black_box(&config)).unwrap())
    });
}

fn bench_process_config_deserialize(c: &mut Criterion) {
    let json = serde_json::to_string(&create_process_config()).unwrap();

    c.bench_function("process_config_deserialize", |b| {
        b.iter(|| {
            let _: ProcessConfig = serde_json::from_str(black_box(&json)).unwrap();
        })
    });
}

fn bench_process_status_serialize(c: &mut Criterion) {
    let status = create_process_status();

    c.bench_function("process_status_serialize", |b| {
        b.iter(|| serde_json::to_string(black_box(&status)).unwrap())
    });
}

fn bench_process_status_deserialize(c: &mut Criterion) {
    let json = serde_json::to_string(&create_process_status()).unwrap();

    c.bench_function("process_status_deserialize", |b| {
        b.iter(|| {
            let _: ProcessStatus = serde_json::from_str(black_box(&json)).unwrap();
        })
    });
}

fn bench_default_manager_spawn(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let pm = DefaultProcessManager;
    let ctx = CapabilityContext::new("bench");

    c.bench_function("default_manager_spawn", |b| {
        b.to_async(&rt).iter(|| {
            pm.spawn(black_box(&ctx), black_box("ls"), black_box(&["-la"] as &[&str]))
        })
    });
}

fn bench_default_manager_events(c: &mut Criterion) {
    let pm = DefaultProcessManager;

    c.bench_function("default_manager_events", |b| {
        b.iter(|| {
            let _rx = pm.events();
        })
    });
}

criterion_group!(
    benches,
    bench_process_config_serialize,
    bench_process_config_deserialize,
    bench_process_status_serialize,
    bench_process_status_deserialize,
    bench_default_manager_spawn,
    bench_default_manager_events,
);
criterion_main!(benches);
