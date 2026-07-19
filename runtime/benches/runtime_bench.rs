//! Benchmarks for the Runtime Platform.
//!
//! Run with: `cargo bench -p ai-os-runtime`
use std::collections::HashMap;

use criterion::{criterion_group, criterion_main, Criterion};

use ai_os_core::bootstrap::PlatformBuilder;

use ai_os_runtime::resource::ResourceUsage;
use ai_os_runtime::scheduler::Scheduler;
use ai_os_runtime::session::SessionManager;
use ai_os_runtime::supervisor::{RestartPolicy, Supervisor};
use ai_os_runtime::task::{Priority, TaskHandle, TaskManager};
use ai_os_runtime::Runtime;

fn build_runtime() -> Runtime {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let app = rt.block_on(async {
        PlatformBuilder::new()
            .without_console_sink()
            .build()
            .await
            .unwrap()
    });
    Runtime::new(app.event_bus().clone(), app.logger().clone())
}

fn bench_scheduler_enqueue_dequeue(c: &mut Criterion) {
    let rt = build_runtime();

    c.bench_function("scheduler_enqueue_dequeue_1000", |b| {
        b.iter(|| {
            let mut handles = Vec::with_capacity(1000);
            for i in 0..1000 {
                let priority = if i % 3 == 0 {
                    Priority::HIGH
                } else if i % 3 == 1 {
                    Priority::NORMAL
                } else {
                    Priority::LOW
                };
                let task = rt.task_manager.create_task(None, priority, HashMap::new());
                handles.push(TaskHandle {
                    task_id: task.id.clone(),
                    priority,
                    session_id: None,
                    created_at: task.created_at,
                });
            }

            for h in &handles {
                rt.scheduler.enqueue(h.clone()).unwrap();
            }

            while rt.scheduler.dequeue().is_some() {}
        })
    });
}

fn bench_session_create_destroy(c: &mut Criterion) {
    let rt = build_runtime();

    c.bench_function("session_create_destroy_100", |b| {
        b.iter(|| {
            let mut ids = Vec::with_capacity(100);
            for _ in 0..100 {
                let session = rt.session_manager.create_session(HashMap::new());
                ids.push(session.id);
            }
            for id in &ids {
                let _ = rt.session_manager.destroy_session(id);
            }
        })
    });
}

fn bench_task_lifecycle(c: &mut Criterion) {
    let rt = build_runtime();

    c.bench_function("task_create_update_1000", |b| {
        b.iter(|| {
            let mut tasks = Vec::with_capacity(1000);
            for _ in 0..1000 {
                let task = rt
                    .task_manager
                    .create_task(None, Priority::NORMAL, HashMap::new());
                tasks.push(task.id);
            }

            for id in &tasks {
                rt.task_manager
                    .update_state(id, ai_os_runtime::task::TaskState::Running)
                    .unwrap();
                rt.task_manager
                    .update_state(id, ai_os_runtime::task::TaskState::Completed)
                    .unwrap();
            }
        })
    });
}

fn bench_resource_tracking(c: &mut Criterion) {
    let rt = build_runtime();

    c.bench_function("resource_track_read_1000", |b| {
        b.iter(|| {
            for i in 0..1000 {
                let id = ai_os_runtime::task::TaskId::new();
                rt.resource_manager
                    .track(
                        &id,
                        ResourceUsage {
                            cpu_percent: i as f64 * 0.1,
                            memory_bytes: i as u64 * 100,
                        },
                    )
                    .unwrap();
                let _ = rt.resource_manager.usage(&id);
            }
        })
    });
}

fn bench_supervisor_record_failure(c: &mut Criterion) {
    let rt = build_runtime();

    c.bench_function("supervisor_record_failure_100", |b| {
        b.iter(|| {
            for _ in 0..100 {
                let id = ai_os_runtime::task::TaskId::new();
                rt.supervisor
                    .supervise(id.clone(), RestartPolicy::OnFailure { max_retries: 5 })
                    .unwrap();
                let _ = rt.supervisor.record_failure(&id, "bench error");
            }
        })
    });
}

criterion_group!(
    benches,
    bench_scheduler_enqueue_dequeue,
    bench_session_create_destroy,
    bench_task_lifecycle,
    bench_resource_tracking,
    bench_supervisor_record_failure,
);
criterion_main!(benches);
