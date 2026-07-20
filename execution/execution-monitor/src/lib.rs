#![forbid(unsafe_code)]

pub mod error;
pub mod monitor;

pub use error::{MonitorError, MonitorResult};
pub use monitor::{DefaultExecutionMonitor, ExecutionWatch, WatchState};
