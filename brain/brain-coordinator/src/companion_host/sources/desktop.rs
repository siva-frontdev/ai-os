use std::sync::Mutex;

use super::ObservationSource;

/// Abstraction over desktop state observation.
///
/// The companion host receives a boxed DesktopProvider during setup;
/// it does not know about OsalDesktopObserver or perception-core.
#[async_trait::async_trait]
pub trait DesktopProvider: std::fmt::Debug + Send + Sync {
    async fn observe_desktop(&self) -> Result<String, String>;
}

/// Polls desktop state through an injected DesktopProvider.
/// Only emits when the summary changes.
#[derive(Debug)]
pub struct DesktopSource {
    provider: Box<dyn DesktopProvider>,
    last_summary: Mutex<Option<String>>,
}

impl DesktopSource {
    pub fn new(provider: Box<dyn DesktopProvider>) -> Self {
        Self {
            provider,
            last_summary: Mutex::new(None),
        }
    }
}

#[async_trait::async_trait]
impl ObservationSource for DesktopSource {
    fn name(&self) -> &'static str {
        "desktop"
    }

    async fn poll(&self) -> Option<String> {
        let summary = match self.provider.observe_desktop().await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("desktop poll failed: {e}");
                return None;
            }
        };

        let mut last = self.last_summary.lock().ok()?;
        if last.as_deref() == Some(&summary) {
            return None;
        }
        *last = Some(summary.clone());
        Some(summary)
    }
}

/// Adapter that wraps an async function as a DesktopProvider.
#[derive(Debug)]
pub struct FnDesktopProvider<F> {
    f: F,
}

impl<F> FnDesktopProvider<F>
where
    F: std::fmt::Debug + Send + Sync,
{
    pub fn new(f: F) -> Self {
        Self { f }
    }
}

#[async_trait::async_trait]
impl<F, Fut> DesktopProvider for FnDesktopProvider<F>
where
    F: std::fmt::Debug + Send + Sync + Fn() -> Fut,
    Fut: std::future::Future<Output = Result<String, String>> + Send,
{
    async fn observe_desktop(&self) -> Result<String, String> {
        (self.f)().await
    }
}
