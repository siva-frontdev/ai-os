//! # Structured Logging
//!
//! The core defines a [`Logger`] trait that decouples the
//! platform from any specific logging backend.  A
//! [`TracingLogger`] implementation bridges to the
//! `tracing` ecosystem so that every structured event is
//! captured in JSON format with timestamps, targets, and
//! arbitrary key-value fields.
//!
//! ## Design decisions
//!
//! * **Abstraction over backend** — services depend on
//!   `dyn Logger`, not on `tracing` directly.
//! * **Hierarchical loggers** — `Logger::child("sub")` creates
//!   a scoped logger that prefixes every message with `sub`.
//! * **Fields** — the `with_field` method returns a new
//!   `Arc<dyn Logger>` that attaches extra structured data to
//!   every record.
//! * **Levels** — `Trace` through `Error` mirror standard
//!   severity.
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::error::CoreError;

// ── Log level ─────────────────────────────────────────────

/// Severity of a log record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Trace => "TRACE",
            LogLevel::Debug => "DEBUG",
            LogLevel::Info => "INFO",
            LogLevel::Warn => "WARN",
            LogLevel::Error => "ERROR",
        }
    }
}

// ── Log record ────────────────────────────────────────────

/// A single structured log entry.
#[derive(Debug, Clone)]
pub struct LogRecord {
    pub level: LogLevel,
    pub message: String,
    pub target: String,
    pub timestamp: DateTime<Utc>,
    pub fields: HashMap<String, Value>,
}

// ── Logger trait ──────────────────────────────────────────

/// Core logging abstraction.
pub trait Logger: Debug + Send + Sync {
    /// Emit a fully-formed record.
    fn log(&self, record: LogRecord);

    /// Convenience: log at `TRACE`.
    fn trace(&self, msg: &str) {
        self.log(LogRecord {
            level: LogLevel::Trace,
            message: msg.to_string(),
            target: self.target().to_string(),
            timestamp: Utc::now(),
            fields: HashMap::new(),
        });
    }

    fn debug(&self, msg: &str) {
        self.log(LogRecord {
            level: LogLevel::Debug,
            message: msg.to_string(),
            target: self.target().to_string(),
            timestamp: Utc::now(),
            fields: HashMap::new(),
        });
    }

    fn info(&self, msg: &str) {
        self.log(LogRecord {
            level: LogLevel::Info,
            message: msg.to_string(),
            target: self.target().to_string(),
            timestamp: Utc::now(),
            fields: HashMap::new(),
        });
    }

    fn warn(&self, msg: &str) {
        self.log(LogRecord {
            level: LogLevel::Warn,
            message: msg.to_string(),
            target: self.target().to_string(),
            timestamp: Utc::now(),
            fields: HashMap::new(),
        });
    }

    fn error(&self, msg: &str) {
        self.log(LogRecord {
            level: LogLevel::Error,
            message: msg.to_string(),
            target: self.target().to_string(),
            timestamp: Utc::now(),
            fields: HashMap::new(),
        });
    }

    /// Return a human-readable target / module path.
    fn target(&self) -> &str;

    /// Create a child logger with `target` appended.
    fn child(&self, target: &str) -> Arc<dyn Logger>;

    /// Return a new logger that attaches `fields` to every record.
    fn with_fields(&self, fields: HashMap<String, Value>) -> Arc<dyn Logger>;

    /// Convenience: attach a single key-value pair.
    fn with_field(&self, key: &str, value: Value) -> Arc<dyn Logger> {
        let mut map = HashMap::new();
        map.insert(key.to_string(), value);
        self.with_fields(map)
    }
}

// ── Sink trait ────────────────────────────────────────────

/// Destination for log records (console, file, external service).
pub trait LogSink: std::fmt::Debug + Send + Sync {
    fn write(&self, record: &LogRecord) -> Result<(), CoreError>;
}

// ── Console sink ──────────────────────────────────────────

/// Writes JSON-formatted logs to stdout.
#[derive(Debug)]
pub struct ConsoleSink;

impl ConsoleSink {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ConsoleSink {
    fn default() -> Self {
        Self::new()
    }
}

impl LogSink for ConsoleSink {
    fn write(&self, record: &LogRecord) -> Result<(), CoreError> {
        let entry = serde_json::json!({
            "level": record.level.as_str(),
            "message": record.message,
            "target": record.target,
            "timestamp": record.timestamp.to_rfc3339(),
            "fields": record.fields,
        });
        println!("{}", serde_json::to_string(&entry).unwrap());
        Ok(())
    }
}

// ── Logger implementations ────────────────────────────────

/// Standard logger that writes to a collection of [`LogSink`]s.
#[derive(Debug, Clone)]
pub struct DefaultLogger {
    target: String,
    fields: HashMap<String, Value>,
    sinks: Vec<Arc<dyn LogSink>>,
}

impl DefaultLogger {
    pub fn new(target: &str) -> Self {
        Self {
            target: target.to_string(),
            fields: HashMap::new(),
            sinks: Vec::new(),
        }
    }

    pub fn with_sink(mut self, sink: Arc<dyn LogSink>) -> Self {
        self.sinks.push(sink);
        self
    }
}

impl Logger for DefaultLogger {
    fn log(&self, mut record: LogRecord) {
        for (k, v) in &self.fields {
            record.fields.insert(k.clone(), v.clone());
        }
        for sink in &self.sinks {
            let _ = sink.write(&record);
        }
    }

    fn target(&self) -> &str {
        &self.target
    }

    fn child(&self, target: &str) -> Arc<dyn Logger> {
        let child_target = if self.target.is_empty() {
            target.to_string()
        } else {
            format!("{}.{}", self.target, target)
        };
        Arc::new(DefaultLogger {
            target: child_target,
            fields: self.fields.clone(),
            sinks: self.sinks.clone(),
        })
    }

    fn with_fields(&self, fields: HashMap<String, Value>) -> Arc<dyn Logger> {
        let mut merged = self.fields.clone();
        merged.extend(fields);
        Arc::new(DefaultLogger {
            target: self.target.clone(),
            fields: merged,
            sinks: self.sinks.clone(),
        })
    }
}

// ── TracingBridge — sends core logs into the tracing ecosystem ──

/// A [`LogSink`] that forwards every record to `tracing`.
///
/// This lets the platform's structured logs appear alongside
/// any library that uses `tracing` directly.
#[derive(Debug)]
pub struct TracingBridge;

impl TracingBridge {
    pub fn new() -> Self {
        Self
    }

    /// Initialise a global `tracing_subscriber` that outputs
    /// JSON and respects `RUST_LOG`.
    pub fn init_global_subscriber() {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .json()
            .try_init();
    }
}

impl Default for TracingBridge {
    fn default() -> Self {
        Self::new()
    }
}

impl LogSink for TracingBridge {
    fn write(&self, record: &LogRecord) -> Result<(), CoreError> {
        match record.level {
            LogLevel::Trace => {
                tracing::event!(
                    tracing::Level::TRACE,
                    target = record.target,
                    message = record.message,
                    fields = ?record.fields,
                )
            }
            LogLevel::Debug => {
                tracing::event!(
                    tracing::Level::DEBUG,
                    target = record.target,
                    message = record.message,
                    fields = ?record.fields,
                )
            }
            LogLevel::Info => {
                tracing::event!(
                    tracing::Level::INFO,
                    target = record.target,
                    message = record.message,
                    fields = ?record.fields,
                )
            }
            LogLevel::Warn => {
                tracing::event!(
                    tracing::Level::WARN,
                    target = record.target,
                    message = record.message,
                    fields = ?record.fields,
                )
            }
            LogLevel::Error => {
                tracing::event!(
                    tracing::Level::ERROR,
                    target = record.target,
                    message = record.message,
                    fields = ?record.fields,
                )
            }
        }
        Ok(())
    }
}

// ── NullLogger (useful for tests) ─────────────────────────

/// A logger that discards everything.
#[derive(Debug)]
pub struct NullLogger;

impl NullLogger {
    pub fn new() -> Self {
        Self
    }

    pub fn boxed() -> Arc<dyn Logger> {
        Arc::new(Self)
    }
}

impl Logger for NullLogger {
    fn log(&self, _record: LogRecord) {}
    fn target(&self) -> &str {
        "null"
    }
    fn child(&self, _target: &str) -> Arc<dyn Logger> {
        Arc::new(NullLogger)
    }
    fn with_fields(&self, _fields: HashMap<String, Value>) -> Arc<dyn Logger> {
        Arc::new(NullLogger)
    }
}

// ── Tests ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_level_as_str() {
        assert_eq!(LogLevel::Trace.as_str(), "TRACE");
        assert_eq!(LogLevel::Info.as_str(), "INFO");
        assert_eq!(LogLevel::Error.as_str(), "ERROR");
    }

    #[test]
    fn default_logger_emits_to_sinks() {
        let sink = Arc::new(TestSink::new());
        let logger = DefaultLogger::new("test").with_sink(sink.clone() as Arc<dyn LogSink>);

        logger.info("hello");
        assert_eq!(sink.records.lock().unwrap().len(), 1);
        assert!(sink.records.lock().unwrap()[0].message.contains("hello"));
    }

    #[test]
    fn child_logger_appends_target() {
        let sink = Arc::new(TestSink::new());
        let parent = DefaultLogger::new("parent").with_sink(sink.clone());
        let child = parent.child("child");

        child.info("test");
        let rec = sink.records.lock().unwrap()[0].clone();
        assert_eq!(rec.target, "parent.child");
    }

    #[test]
    fn with_fields_attaches_data() {
        let sink = Arc::new(TestSink::new());
        let logger = DefaultLogger::new("test")
            .with_sink(sink.clone())
            .with_field("user_id", Value::String("abc".into()));

        logger.info("login");
        let rec = sink.records.lock().unwrap()[0].clone();
        assert_eq!(
            rec.fields.get("user_id"),
            Some(&Value::String("abc".into()))
        );
    }

    #[test]
    fn null_logger_does_not_panic() {
        let logger = NullLogger;
        logger.info("should not crash");
        logger.child("sub").info("still fine");
        logger.with_field("x", Value::Null).info("ok");
    }

    // ── Test helper ───────────────────────────────────────

    #[derive(Debug)]
    struct TestSink {
        records: std::sync::Mutex<Vec<LogRecord>>,
    }

    impl TestSink {
        fn new() -> Self {
            Self {
                records: std::sync::Mutex::new(Vec::new()),
            }
        }
    }

    impl LogSink for TestSink {
        fn write(&self, record: &LogRecord) -> Result<(), CoreError> {
            self.records.lock().unwrap().push(record.clone());
            Ok(())
        }
    }
}
