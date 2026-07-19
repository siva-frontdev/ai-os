use std::collections::HashMap;
use std::time::Duration;

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use osal_terminal::{
    CommandConfig, CommandOutput, PtyConfig, TerminalOutput, TerminalOutputEventType,
};

fn bench_command_config_serde(c: &mut Criterion) {
    let config = CommandConfig {
        command: "ls".into(),
        args: vec!["-la".into(), "/tmp".into()],
        env: [("PATH".into(), "/usr/bin".into())].into(),
        working_dir: Some("/tmp".into()),
        timeout: Some(Duration::from_secs(30)),
        env_clean: false,
        capture_output: true,
    };

    c.bench_function("command_config_serialize", |b| {
        b.iter(|| serde_json::to_string(black_box(&config)))
    });

    let json = serde_json::to_string(&config).unwrap();
    c.bench_function("command_config_deserialize", |b| {
        b.iter(|| serde_json::from_str::<CommandConfig>(black_box(&json)))
    });
}

fn bench_command_output_serde(c: &mut Criterion) {
    let output = CommandOutput {
        exit_status: 0,
        stdout: vec![b'x'; 4096],
        stderr: Vec::new(),
        duration: Duration::from_millis(42),
    };

    c.bench_function("command_output_serialize", |b| {
        b.iter(|| serde_json::to_string(black_box(&output)))
    });

    let json = serde_json::to_string(&output).unwrap();
    c.bench_function("command_output_deserialize", |b| {
        b.iter(|| serde_json::from_str::<CommandOutput>(black_box(&json)))
    });
}

fn bench_pty_config_serde(c: &mut Criterion) {
    let config = PtyConfig {
        command: "bash".into(),
        args: vec!["-c".into(), "echo hello".into()],
        env: HashMap::new(),
        working_dir: None,
        cols: 80,
        rows: 24,
        term_env: "xterm-256color".into(),
    };

    c.bench_function("pty_config_serialize", |b| {
        b.iter(|| serde_json::to_string(black_box(&config)))
    });

    let json = serde_json::to_string(&config).unwrap();
    c.bench_function("pty_config_deserialize", |b| {
        b.iter(|| serde_json::from_str::<PtyConfig>(black_box(&json)))
    });
}

fn bench_terminal_output_serde(c: &mut Criterion) {
    use chrono::Utc;
    use osal_core::SessionId;

    let output = TerminalOutput {
        session_id: SessionId("sess-1".into()),
        data: vec![b'x'; 1024],
        timestamp: Utc::now(),
        event_type: TerminalOutputEventType::Stdout,
    };

    c.bench_function("terminal_output_serialize", |b| {
        b.iter(|| serde_json::to_string(black_box(&output)))
    });

    let json = serde_json::to_string(&output).unwrap();
    c.bench_function("terminal_output_deserialize", |b| {
        b.iter(|| serde_json::from_str::<TerminalOutput>(black_box(&json)))
    });
}

fn bench_pty_config_default(c: &mut Criterion) {
    c.bench_function("pty_config_default", |b| b.iter(|| PtyConfig::default()));
}

fn bench_command_config_new(c: &mut Criterion) {
    c.bench_function("command_config_new", |b| {
        b.iter(|| CommandConfig {
            command: black_box("ls".into()),
            args: black_box(vec!["-la".into()]),
            env: black_box(HashMap::new()),
            working_dir: black_box(None),
            timeout: black_box(None),
            env_clean: black_box(false),
            capture_output: black_box(true),
        })
    });
}

criterion_group!(
    benches,
    bench_command_config_serde,
    bench_command_output_serde,
    bench_pty_config_serde,
    bench_terminal_output_serde,
    bench_pty_config_default,
    bench_command_config_new,
);
criterion_main!(benches);
