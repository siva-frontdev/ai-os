//! # Health Monitoring
//!
//! On-demand and periodic health checks for every platform
//! service.  Results are cached and available via the
//! [`HealthMonitor`] trait.
//!
//! ## Design decisions
//!
//! * **Pull-based** — checks run explicitly (or on a timer),
//!   not pushed from the service.
//! * **Three-state** — [`HealthStatus::Healthy`],
//!   [`HealthStatus::Degraded`], [`HealthStatus::Unhealthy`].
//! * **Read-through cache** — the latest [`HealthReport`] for
//!   each check is cached so a management API can query without
//!   re-running.
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use tokio::time;

use crate::error::CoreError;

// ── Health status ─────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HealthStatus {
    Healthy,
    Degraded { message: String },
    Unhealthy { message: String },
}

impl HealthStatus {
    pub fn is_healthy(&self) -> bool {
        matches!(self, HealthStatus::Healthy)
    }

    pub fn is_degraded(&self) -> bool {
        matches!(self, HealthStatus::Degraded { .. })
    }

    pub fn is_unhealthy(&self) -> bool {
        matches!(self, HealthStatus::Unhealthy { .. })
    }

    pub fn summary(&self) -> &str {
        match self {
            HealthStatus::Healthy => "healthy",
            HealthStatus::Degraded { .. } => "degraded",
            HealthStatus::Unhealthy { .. } => "unhealthy",
        }
    }
}

// ── Health report ─────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct HealthReport {
    pub service_name: String,
    pub status: HealthStatus,
    pub checked_at: DateTime<Utc>,
    pub details: HashMap<String, String>,
}

// ── HealthCheck trait ─────────────────────────────────────

#[async_trait]
pub trait HealthCheck: Debug + Send + Sync + 'static {
    fn name(&self) -> &str;
    async fn check(&self) -> HealthStatus;
}

// ── HealthMonitor trait ───────────────────────────────────

#[async_trait]
pub trait HealthMonitor: Debug + Send + Sync {
    fn register(&self, check: Arc<dyn HealthCheck>) -> Result<(), CoreError>;
    async fn run_checks(&self) -> Vec<HealthReport>;
    fn start_periodic(self: Arc<Self>, interval: Duration) -> tokio::task::JoinHandle<()>;
    fn latest(&self, service: &str) -> Option<HealthReport>;
    fn all_latest(&self) -> Vec<HealthReport>;
}

// ── Implementation ────────────────────────────────────────

#[derive(Debug)]
pub struct DefaultHealthMonitor {
    checks: RwLock<Vec<Arc<dyn HealthCheck>>>,
    cache: RwLock<HashMap<String, HealthReport>>,
}

impl DefaultHealthMonitor {
    pub fn new() -> Self {
        Self {
            checks: RwLock::new(Vec::new()),
            cache: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for DefaultHealthMonitor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl HealthMonitor for DefaultHealthMonitor {
    fn register(&self, check: Arc<dyn HealthCheck>) -> Result<(), CoreError> {
        self.checks
            .write()
            .map_err(|_| CoreError::LockPoisoned)?
            .push(check);
        Ok(())
    }

    async fn run_checks(&self) -> Vec<HealthReport> {
        let checks: Vec<Arc<dyn HealthCheck>> = self
            .checks
            .read()
            .map(|g| g.clone())
            .unwrap_or_default();

        let mut reports = Vec::with_capacity(checks.len());
        for check in &checks {
            let name = check.name().to_string();
            let status = check.check().await;
            let report = HealthReport {
                service_name: name.clone(),
                status,
                checked_at: Utc::now(),
                details: HashMap::new(),
            };

            // Update cache (best-effort)
            if let Ok(mut cache) = self.cache.write() {
                cache.insert(name, report.clone());
            }

            reports.push(report);
        }
        reports
    }

    fn start_periodic(self: Arc<Self>, interval: Duration) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut ticker = time::interval(interval);
            loop {
                ticker.tick().await;
                self.run_checks().await;
            }
        })
    }

    fn latest(&self, service: &str) -> Option<HealthReport> {
        self.cache.read().ok()?.get(service).cloned()
    }

    fn all_latest(&self) -> Vec<HealthReport> {
        self.cache
            .read()
            .map(|g| g.values().cloned().collect())
            .unwrap_or_default()
    }
}

// ── Pre-built checks ──────────────────────────────────────

#[derive(Debug)]
pub struct HealthyCheck {
    name: String,
}

impl HealthyCheck {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
        }
    }
}

#[async_trait]
impl HealthCheck for HealthyCheck {
    fn name(&self) -> &str {
        &self.name
    }

    async fn check(&self) -> HealthStatus {
        HealthStatus::Healthy
    }
}

// ── Tests ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct AlwaysHealthy;

    #[async_trait]
    impl HealthCheck for AlwaysHealthy {
        fn name(&self) -> &str {
            "always_ok"
        }
        async fn check(&self) -> HealthStatus {
            HealthStatus::Healthy
        }
    }

    #[derive(Debug)]
    struct AlwaysUnhealthy;

    #[async_trait]
    impl HealthCheck for AlwaysUnhealthy {
        fn name(&self) -> &str {
            "always_bad"
        }
        async fn check(&self) -> HealthStatus {
            HealthStatus::Unhealthy {
                message: "simulated failure".into(),
            }
        }
    }

    #[tokio::test]
    async fn register_and_run_single() {
        let monitor = DefaultHealthMonitor::new();
        monitor.register(Arc::new(AlwaysHealthy)).unwrap();
        let reports = monitor.run_checks().await;
        assert_eq!(reports.len(), 1);
        assert!(reports[0].status.is_healthy());
        assert_eq!(reports[0].service_name, "always_ok");
    }

    #[tokio::test]
    async fn multiple_checks() {
        let monitor = DefaultHealthMonitor::new();
        monitor.register(Arc::new(AlwaysHealthy)).unwrap();
        monitor.register(Arc::new(AlwaysUnhealthy)).unwrap();

        let reports = monitor.run_checks().await;
        assert_eq!(reports.len(), 2);
        let ok = reports.iter().filter(|r| r.status.is_healthy()).count();
        let bad = reports.iter().filter(|r| r.status.is_unhealthy()).count();
        assert_eq!(ok, 1);
        assert_eq!(bad, 1);
    }

    #[tokio::test]
    async fn cache_populated_after_run() {
        let monitor = DefaultHealthMonitor::new();
        monitor.register(Arc::new(AlwaysHealthy)).unwrap();
        assert!(monitor.latest("always_ok").is_none());
        monitor.run_checks().await;
        assert!(monitor.latest("always_ok").is_some());
        assert!(monitor.latest("always_ok").unwrap().status.is_healthy());
    }

    #[test]
    fn health_status_methods() {
        assert!(HealthStatus::Healthy.is_healthy());
        assert!(HealthStatus::Degraded { message: "x".into() }.is_degraded());
        assert!(HealthStatus::Unhealthy { message: "x".into() }.is_unhealthy());
        assert_eq!(HealthStatus::Unhealthy { message: "x".into() }.summary(), "unhealthy");
    }
}
