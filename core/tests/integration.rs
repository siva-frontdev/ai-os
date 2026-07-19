//! Integration tests for the core platform.
use std::sync::Arc;

use ai_os_core::bootstrap::PlatformBuilder;
use ai_os_core::config::ConfigExt;
use ai_os_core::error::CoreError;
use ai_os_core::events::{typed_subscribe, EventHandler, StartedEvent};
use ai_os_core::health::{HealthCheck, HealthStatus};
use ai_os_core::lifecycle::{Service, ServiceState};
use async_trait::async_trait;
use tokio::sync::Mutex;

#[derive(Debug)]
struct HistoryService {
    name: String,
    history: Arc<Mutex<Vec<String>>>,
}

#[async_trait]
impl Service for HistoryService {
    fn name(&self) -> &str {
        &self.name
    }
    async fn start(&self) -> Result<(), CoreError> {
        self.history
            .lock()
            .await
            .push(format!("{}:start", self.name));
        Ok(())
    }
    async fn stop(&self) -> Result<(), CoreError> {
        self.history
            .lock()
            .await
            .push(format!("{}:stop", self.name));
        Ok(())
    }
}

#[tokio::test]
async fn bootstrap_creates_all_subsystems() {
    let app = PlatformBuilder::new()
        .without_console_sink()
        .build()
        .await
        .expect("bootstrap should succeed");

    // Verify all subsystems are accessible (not null)
    app.logger().child("test").info("bootstrap ok");
    let _ = app.event_bus();
    let _ = app.health();
    let _ = app.lifecycle();
    let _ = app.registry();
    let _ = app.config();
}

#[tokio::test]
async fn service_start_stop_via_application() {
    let history = Arc::new(Mutex::new(Vec::new()));
    let svc: Arc<dyn Service> = Arc::new(HistoryService {
        name: "test-svc".into(),
        history: history.clone(),
    });

    let app = PlatformBuilder::new()
        .without_console_sink()
        .build()
        .await
        .unwrap();

    app.lifecycle().register(svc).await.unwrap();
    assert_eq!(
        app.lifecycle().state("test-svc"),
        Some(ServiceState::Created)
    );

    app.run().await.unwrap();
    assert!(app.is_running());
    assert_eq!(
        app.lifecycle().state("test-svc"),
        Some(ServiceState::Running)
    );

    app.shutdown().await.unwrap();
    assert!(!app.is_running());
    assert_eq!(
        app.lifecycle().state("test-svc"),
        Some(ServiceState::Stopped)
    );

    let hist = history.lock().await;
    assert_eq!(hist.len(), 2);
    assert_eq!(hist[0], "test-svc:start");
    assert_eq!(hist[1], "test-svc:stop");
}

#[tokio::test]
async fn event_bus_fires_started_event() {
    let app = PlatformBuilder::new()
        .without_console_sink()
        .build()
        .await
        .unwrap();

    let fired = Arc::new(std::sync::atomic::AtomicBool::new(false));

    #[derive(Debug)]
    struct TestHandler {
        flag: Arc<std::sync::atomic::AtomicBool>,
    }

    #[async_trait]
    impl EventHandler<StartedEvent> for TestHandler {
        async fn handle(&self, _event: &StartedEvent) -> Result<(), CoreError> {
            self.flag.store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        }
    }

    typed_subscribe::<StartedEvent, TestHandler>(
        app.event_bus().as_ref(),
        Arc::new(TestHandler {
            flag: fired.clone(),
        }),
    )
    .unwrap();

    app.run().await.unwrap();
    assert!(fired.load(std::sync::atomic::Ordering::SeqCst));
    app.shutdown().await.unwrap();
}

#[tokio::test]
async fn health_checks_are_runnable() {
    let app = PlatformBuilder::new()
        .without_console_sink()
        .build()
        .await
        .unwrap();

    #[derive(Debug)]
    struct OkCheck;
    #[async_trait]
    impl HealthCheck for OkCheck {
        fn name(&self) -> &str {
            "test-check"
        }
        async fn check(&self) -> HealthStatus {
            HealthStatus::Healthy
        }
    }

    app.health().register(Arc::new(OkCheck)).unwrap();
    let reports = app.health().run_checks().await;
    assert_eq!(reports.len(), 1);
    assert!(reports[0].status.is_healthy());
}

#[tokio::test]
async fn config_is_queryable() {
    use ai_os_core::config::InMemoryConfigProvider;

    let provider = InMemoryConfigProvider::from_str(r#"{"key": "val"}"#).unwrap();
    let app = PlatformBuilder::new()
        .without_console_sink()
        .with_config_provider(Box::new(provider))
        .build()
        .await
        .unwrap();

    let val: Option<String> = app.config().get("key").unwrap();
    assert_eq!(val, Some("val".into()));
}

#[tokio::test]
async fn logger_child_does_not_panic() {
    let app = PlatformBuilder::new()
        .without_console_sink()
        .build()
        .await
        .unwrap();
    app.logger().child("test").info("no panic");
}

#[tokio::test]
async fn services_start_and_stop_in_correct_order() {
    let history = Arc::new(Mutex::new(Vec::new()));
    let app = PlatformBuilder::new()
        .without_console_sink()
        .build()
        .await
        .unwrap();

    app.lifecycle()
        .register(Arc::new(HistoryService {
            name: "first".into(),
            history: history.clone(),
        }))
        .await
        .unwrap();
    app.lifecycle()
        .register(Arc::new(HistoryService {
            name: "second".into(),
            history: history.clone(),
        }))
        .await
        .unwrap();

    app.run().await.unwrap();
    app.shutdown().await.unwrap();

    let hist = history.lock().await;
    assert_eq!(hist[0], "first:start");
    assert_eq!(hist[1], "second:start");
    assert_eq!(hist[2], "second:stop");
    assert_eq!(hist[3], "first:stop");
}
