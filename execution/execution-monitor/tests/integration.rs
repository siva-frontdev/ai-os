use std::time::Duration;

use execution_core::traits::ExecutionMonitor;
use execution_core::types::*;
use execution_monitor::DefaultExecutionMonitor;

#[tokio::test]
async fn test_start_watch_tracks_execution() {
    let monitor = DefaultExecutionMonitor::without_event_bus();
    let id = ExecutionId::new();
    let budget = ExecutionBudget::new(60_000);
    let (cancel_tx, _cancel_rx) = tokio::sync::oneshot::channel();

    monitor.start_watch(id, budget, cancel_tx).await;

    let metrics = monitor.current_metrics(&id).await.unwrap();
    assert!(metrics.is_some(), "watch should exist after start");
    assert_eq!(metrics.unwrap(), ExecutionMetrics::default());
}

#[tokio::test]
async fn test_stop_watch_removes_execution() {
    let monitor = DefaultExecutionMonitor::without_event_bus();
    let id = ExecutionId::new();
    let budget = ExecutionBudget::new(60_000);
    let (cancel_tx, _cancel_rx) = tokio::sync::oneshot::channel();

    monitor.start_watch(id, budget, cancel_tx).await;
    monitor.stop_watch(&id).await;

    let metrics = monitor.current_metrics(&id).await.unwrap();
    assert!(metrics.is_none(), "watch should be removed after stop");
}

#[tokio::test]
async fn test_heartbeat_updates_metrics() {
    let monitor = DefaultExecutionMonitor::without_event_bus();
    let id = ExecutionId::new();
    let budget = ExecutionBudget::new(60_000);
    let (cancel_tx, _cancel_rx) = tokio::sync::oneshot::channel();

    monitor.start_watch(id, budget, cancel_tx).await;

    let reported = ExecutionMetrics {
        wall_clock_ms: 100,
        cpu_ms: 50,
        peak_memory_bytes: 1024,
        io_read_bytes: 512,
        io_write_bytes: 256,
    };
    monitor.report_heartbeat(&id, reported).await;

    let stored = monitor.current_metrics(&id).await.unwrap().unwrap();
    assert_eq!(stored.wall_clock_ms, 100);
    assert_eq!(stored.cpu_ms, 50);
    assert_eq!(stored.peak_memory_bytes, 1024);
    assert_eq!(stored.io_read_bytes, 512);
    assert_eq!(stored.io_write_bytes, 256);
}

#[tokio::test]
async fn test_heartbeat_accumulates_max_metrics() {
    let monitor = DefaultExecutionMonitor::without_event_bus();
    let id = ExecutionId::new();
    let budget = ExecutionBudget::new(60_000);
    let (cancel_tx, _cancel_rx) = tokio::sync::oneshot::channel();

    monitor.start_watch(id, budget, cancel_tx).await;

    monitor
        .report_heartbeat(
            &id,
            ExecutionMetrics {
                wall_clock_ms: 100,
                cpu_ms: 50,
                peak_memory_bytes: 1024,
                io_read_bytes: 512,
                io_write_bytes: 256,
            },
        )
        .await;

    monitor
        .report_heartbeat(
            &id,
            ExecutionMetrics {
                wall_clock_ms: 50,
                cpu_ms: 75,
                peak_memory_bytes: 2048,
                io_read_bytes: 256,
                io_write_bytes: 512,
            },
        )
        .await;

    let stored = monitor.current_metrics(&id).await.unwrap().unwrap();
    assert_eq!(stored.wall_clock_ms, 100);
    assert_eq!(stored.cpu_ms, 75);
    assert_eq!(stored.peak_memory_bytes, 2048);
    assert_eq!(stored.io_read_bytes, 512);
    assert_eq!(stored.io_write_bytes, 512);
}

#[tokio::test]
async fn test_timeout_triggers() {
    tokio::time::pause();

    let monitor = DefaultExecutionMonitor::without_event_bus();
    let id = ExecutionId::new();
    let budget = ExecutionBudget::new(10);
    let (cancel_tx, mut cancel_rx) = tokio::sync::oneshot::channel();

    monitor.start_watch(id, budget, cancel_tx).await;

    tokio::time::advance(Duration::from_millis(50)).await;
    tokio::task::yield_now().await;

    match cancel_rx.try_recv() {
        Ok(reason) => assert_eq!(reason, CancelReason::Timeout),
        Err(e) => panic!("expected Timeout, got try_recv error: {:?}", e),
    }
}

#[tokio::test]
async fn test_stop_before_timeout_prevents_firing() {
    tokio::time::pause();

    let monitor = DefaultExecutionMonitor::without_event_bus();
    let id = ExecutionId::new();
    let budget = ExecutionBudget::new(60_000);
    let (cancel_tx, mut cancel_rx) = tokio::sync::oneshot::channel();

    monitor.start_watch(id, budget, cancel_tx).await;
    monitor.stop_watch(&id).await;

    tokio::time::advance(Duration::from_secs(120)).await;
    tokio::task::yield_now().await;

    match cancel_rx.try_recv() {
        Err(tokio::sync::oneshot::error::TryRecvError::Closed) => {}
        other => panic!("expected Closed, got {:?}", other),
    }
}

#[tokio::test]
async fn test_health_with_no_watches_is_healthy() {
    let monitor = DefaultExecutionMonitor::without_event_bus();
    let result = monitor.health().await;
    assert!(result.is_ok(), "health should be ok with no watches");
}

#[tokio::test]
async fn test_report_progress_does_not_panic() {
    let monitor = DefaultExecutionMonitor::without_event_bus();
    let id = ExecutionId::new();
    let budget = ExecutionBudget::new(60_000);
    let (cancel_tx, _cancel_rx) = tokio::sync::oneshot::channel();

    monitor.start_watch(id, budget, cancel_tx).await;
    monitor
        .report_progress(&id, 0.5, "halfway there".into())
        .await;
}

#[tokio::test]
async fn test_metrics_for_nonexistent_watch() {
    let monitor = DefaultExecutionMonitor::without_event_bus();
    let id = ExecutionId::new();
    let result = monitor.current_metrics(&id).await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_heartbeat_nonexistent_watch() {
    let monitor = DefaultExecutionMonitor::without_event_bus();
    let id = ExecutionId::new();
    monitor
        .report_heartbeat(
            &id,
            ExecutionMetrics {
                wall_clock_ms: 100,
                cpu_ms: 50,
                peak_memory_bytes: 1024,
                io_read_bytes: 512,
                io_write_bytes: 256,
            },
        )
        .await;
}
