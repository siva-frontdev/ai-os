use std::sync::Arc;

use async_trait::async_trait;
use osal_capabilities::CapabilityContext;
use osal_core::KernelFacade;
use perception_core::{
    DesktopObservationProvider, DesktopState, PerceptionError, PerceptionResult, WindowInfo,
};

/// Observes desktop state through OSAL interfaces.
///
/// This is the single source of desktop observation for the platform.
/// It replaces DesktopObserver from the Desktop Agent.
pub struct OsalDesktopObserver {
    kernel: Arc<KernelFacade>,
    ctx: CapabilityContext,
}

impl std::fmt::Debug for OsalDesktopObserver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OsalDesktopObserver")
            .field("ctx", &self.ctx)
            .finish_non_exhaustive()
    }
}

impl OsalDesktopObserver {
    pub fn new(kernel: Arc<KernelFacade>) -> Self {
        Self {
            kernel,
            ctx: CapabilityContext::new("perception-desktop"),
        }
    }
}

#[async_trait]
impl DesktopObservationProvider for OsalDesktopObserver {
    async fn observe(&self) -> PerceptionResult<DesktopState> {
        let windows = self
            .kernel
            .windows
            .list_windows(&self.ctx)
            .await
            .map_err(|e| PerceptionError::osal(e.to_string()))?
            .into_iter()
            .map(WindowInfo::from)
            .collect();

        let focused_window = self
            .kernel
            .windows
            .focused_window(&self.ctx)
            .await
            .map_err(|e| PerceptionError::osal(e.to_string()))?
            .map(WindowInfo::from);

        let cursor_position = self
            .kernel
            .input
            .cursor_position(&self.ctx)
            .await
            .map_err(|e| PerceptionError::osal(e.to_string()))?;

        let clipboard = self
            .kernel
            .clipboard
            .get_text(&self.ctx)
            .await
            .map_err(|e| PerceptionError::osal(e.to_string()))?;

        let screen_dimensions = self
            .kernel
            .windows
            .screen_dimensions(&self.ctx)
            .await
            .map_err(|e| PerceptionError::osal(e.to_string()))?;

        Ok(DesktopState {
            windows,
            focused_window,
            cursor_position,
            clipboard,
            screen_dimensions,
        })
    }
}
