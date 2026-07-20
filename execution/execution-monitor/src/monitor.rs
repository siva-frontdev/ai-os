use std::collections::HashMap;
use std::sync::RwLock;

use async_trait::async_trait;
use execution_core::event::ExecutionEvent;
use execution_core::traits::ExecutionMonitor;
use execution_core::types::*;
use execution_core::ExecutionResult;
use memory_core::Timestamp;
use tokio::sync::{mpsc, oneshot};
use tokio::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchState {
    Active,
    Completing,
    Completed,
    TimedOut,
    Cancelled,
}

#[derive(Debug)]
pub struct ExecutionWatch {
    pub execution_id: ExecutionId,
    pub state: WatchState,
    pub deadline: Option<Instant>,
    pub cancel_tx: Option<oneshot::Sender<CancelReason>>,
    pub metrics: ExecutionMetrics,
    pub created_at: Instant,
    pub last_heartbeat: Instant,
    pub timeout_handle: Option<tokio::task::JoinHandle<()>>,
}

#[derive(Debug)]
pub struct DefaultExecutionMonitor {
    watches: RwLock<HashMap<ExecutionId, ExecutionWatch>>,
    event_bus: Option<mpsc::Sender<ExecutionEvent>>,
}

impl DefaultExecutionMonitor {
    pub fn new(event_bus: Option<mpsc::Sender<ExecutionEvent>>) -> Self {
        Self {
            watches: RwLock::new(HashMap::new()),
            event_bus,
        }
    }

    pub fn without_event_bus() -> Self {
        Self::new(None)
    }
}

impl Default for DefaultExecutionMonitor {
    fn default() -> Self {
        Self::new(None)
    }
}

#[async_trait]
impl ExecutionMonitor for DefaultExecutionMonitor {
    async fn start_watch(
        &self,
        execution_id: ExecutionId,
        budget: ExecutionBudget,
        cancel_tx: oneshot::Sender<CancelReason>,
    ) {
        let deadline = if budget.timeout_ms > 0 {
            Some(Instant::now() + Duration::from_millis(budget.timeout_ms))
        } else {
            None
        };

        let timeout_handle = if let Some(deadline) = deadline {
            let event_bus = self.event_bus.clone();
            Some(tokio::spawn(timeout_task(
                execution_id,
                deadline,
                budget.timeout_ms,
                cancel_tx,
                event_bus,
            )))
        } else {
            None
        };

        let watch = ExecutionWatch {
            execution_id,
            state: WatchState::Active,
            deadline,
            cancel_tx: None,
            metrics: ExecutionMetrics::default(),
            created_at: Instant::now(),
            last_heartbeat: Instant::now(),
            timeout_handle,
        };

        if let Ok(mut watches) = self.watches.write() {
            watches.insert(execution_id, watch);
        }
    }

    async fn stop_watch(&self, execution_id: &ExecutionId) {
        if let Ok(mut watches) = self.watches.write() {
            if let Some(watch) = watches.remove(execution_id) {
                if let Some(handle) = watch.timeout_handle {
                    handle.abort();
                }
            }
        }
    }

    async fn report_heartbeat(&self, execution_id: &ExecutionId, metrics: ExecutionMetrics) {
        if let Ok(mut watches) = self.watches.write() {
            if let Some(watch) = watches.get_mut(execution_id) {
                watch.last_heartbeat = Instant::now();
                watch.metrics.wall_clock_ms =
                    watch.metrics.wall_clock_ms.max(metrics.wall_clock_ms);
                watch.metrics.cpu_ms = watch.metrics.cpu_ms.max(metrics.cpu_ms);
                watch.metrics.peak_memory_bytes = watch
                    .metrics
                    .peak_memory_bytes
                    .max(metrics.peak_memory_bytes);
                watch.metrics.io_read_bytes =
                    watch.metrics.io_read_bytes.max(metrics.io_read_bytes);
                watch.metrics.io_write_bytes =
                    watch.metrics.io_write_bytes.max(metrics.io_write_bytes);
            }
        }
    }

    async fn report_progress(&self, _execution_id: &ExecutionId, _progress: f64, _message: String) {
    }

    async fn current_metrics(
        &self,
        execution_id: &ExecutionId,
    ) -> ExecutionResult<Option<ExecutionMetrics>> {
        let watches = self
            .watches
            .read()
            .map_err(|_| execution_core::ExecutionError::NotFound("lock poisoned".into()))?;
        Ok(watches.get(execution_id).map(|w| w.metrics))
    }

    async fn health(&self) -> ExecutionResult<()> {
        let watches = self
            .watches
            .read()
            .map_err(|_| execution_core::ExecutionError::NotFound("lock poisoned".into()))?;

        let now = Instant::now();
        for watch in watches.values() {
            if watch.state == WatchState::Active {
                if let Some(deadline) = watch.deadline {
                    let timeout_duration = deadline.saturating_duration_since(watch.created_at);
                    let max_idle = timeout_duration * 2;
                    let elapsed = now.saturating_duration_since(watch.last_heartbeat);
                    if elapsed > max_idle {
                        let elapsed_ms =
                            now.saturating_duration_since(watch.created_at).as_millis() as u64;
                        return Err(execution_core::ExecutionError::TimeoutExceeded { elapsed_ms });
                    }
                }
            }
        }
        Ok(())
    }
}

async fn timeout_task(
    execution_id: ExecutionId,
    deadline: Instant,
    timeout_ms: u64,
    cancel_tx: oneshot::Sender<CancelReason>,
    event_bus: Option<mpsc::Sender<ExecutionEvent>>,
) {
    let start = Instant::now();
    if deadline > start {
        tokio::time::sleep(deadline - start).await;
    }
    let elapsed_ms = start.elapsed().as_millis() as u64;

    if let Some(bus) = event_bus {
        let event = ExecutionEvent::TimedOut(execution_core::event::ExecutionTimedOut {
            execution_id,
            timeout_ms,
            elapsed_ms,
            partial_output: false,
            timestamp: Timestamp::now(),
        });
        let _ = bus.try_send(event);
    }

    let _ = cancel_tx.send(CancelReason::Timeout);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_watch_state_transitions() {
        assert_eq!(WatchState::Active as u8, 0);
        assert_ne!(WatchState::Completing, WatchState::Completed);
        assert_ne!(WatchState::TimedOut, WatchState::Cancelled);
    }

    #[test]
    fn test_default_monitor_creation() {
        let monitor = DefaultExecutionMonitor::default();
        let watches = monitor.watches.read().unwrap();
        assert!(watches.is_empty());
    }

    #[test]
    fn test_execution_watch_debug() {
        let id = ExecutionId::new();
        let watch = ExecutionWatch {
            execution_id: id,
            state: WatchState::Active,
            deadline: None,
            cancel_tx: None,
            metrics: ExecutionMetrics::default(),
            created_at: Instant::now(),
            last_heartbeat: Instant::now(),
            timeout_handle: None,
        };
        let debug = format!("{:?}", watch);
        assert!(debug.contains("execution_id"));
    }

    #[test]
    fn test_new_with_event_bus() {
        let (tx, _rx) = mpsc::channel(16);
        let monitor = DefaultExecutionMonitor::new(Some(tx));
        assert!(monitor.event_bus.is_some());
    }

    #[test]
    fn test_without_event_bus() {
        let monitor = DefaultExecutionMonitor::without_event_bus();
        assert!(monitor.event_bus.is_none());
    }

    #[tokio::test]
    async fn test_start_watch_stores_watch() {
        let monitor = DefaultExecutionMonitor::without_event_bus();
        let id = ExecutionId::new();
        let budget = ExecutionBudget::new(60_000);
        let (cancel_tx, _) = oneshot::channel();

        monitor.start_watch(id, budget, cancel_tx).await;

        let watches = monitor.watches.read().unwrap();
        assert!(watches.contains_key(&id));
    }

    #[tokio::test]
    async fn test_stop_watch_removes_and_aborts() {
        let monitor = DefaultExecutionMonitor::without_event_bus();
        let id = ExecutionId::new();
        let budget = ExecutionBudget::new(60_000);
        let (cancel_tx, _) = oneshot::channel();

        monitor.start_watch(id, budget, cancel_tx).await;
        monitor.stop_watch(&id).await;

        let watches = monitor.watches.read().unwrap();
        assert!(!watches.contains_key(&id));
    }

    #[tokio::test]
    async fn test_heartbeat_updates_last_heartbeat() {
        let monitor = DefaultExecutionMonitor::without_event_bus();
        let id = ExecutionId::new();
        let budget = ExecutionBudget::new(60_000);
        let (cancel_tx, _) = oneshot::channel();

        monitor.start_watch(id, budget, cancel_tx).await;

        let before = {
            let watches = monitor.watches.read().unwrap();
            watches.get(&id).unwrap().last_heartbeat
        };

        tokio::time::sleep(Duration::from_millis(10)).await;

        monitor
            .report_heartbeat(&id, ExecutionMetrics::default())
            .await;

        let after = {
            let watches = monitor.watches.read().unwrap();
            watches.get(&id).unwrap().last_heartbeat
        };

        assert!(after > before);
    }
}
