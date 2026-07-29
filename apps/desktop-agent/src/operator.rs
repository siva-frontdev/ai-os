use std::collections::HashMap;
use std::sync::Arc;

use osal_capabilities::CapabilityContext;
use osal_core::KernelFacade;
use perception_coordinator::PerceptionCoordinator;
use perception_core::DesktopState;

use crate::strategy::{ActionStrategy, StrategySelector};
use crate::verify::{VerificationEngine, VerificationResult};

#[derive(Debug, Clone)]
pub struct Action {
    pub capability_id: String,
    pub params: HashMap<String, String>,
}

impl Action {
    pub fn new(capability_id: impl Into<String>) -> Self {
        Self {
            capability_id: capability_id.into(),
            params: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ActionResult {
    pub capability_id: String,
    pub success: bool,
    pub strategy_used: String,
    pub attempts: u32,
    pub before_state: DesktopState,
    pub after_state: DesktopState,
    pub verification: VerificationResult,
    pub error: Option<String>,
}

pub struct DesktopOperator {
    kernel: Arc<KernelFacade>,
    perception: Arc<dyn PerceptionCoordinator>,
    ctx: CapabilityContext,
}

impl DesktopOperator {
    pub fn new(kernel: Arc<KernelFacade>, perception: Arc<dyn PerceptionCoordinator>) -> Self {
        Self {
            ctx: CapabilityContext::new("desktop-operator"),
            kernel,
            perception,
        }
    }

    async fn observe(&self) -> DesktopState {
        self.perception.observe_desktop().await.unwrap_or_default()
    }

    async fn execute_strategy(&self, strategy: &ActionStrategy) -> Result<String, String> {
        match strategy {
            ActionStrategy::FocusWindow { title, partial } => {
                self.kernel
                    .windows
                    .focus_window(&self.ctx, title, *partial)
                    .await
                    .map_err(|e| format!("focus failed: {}", e))?;
                Ok(format!("focused window '{}'", title))
            }

            ActionStrategy::CloseWindow { title } => {
                self.kernel
                    .windows
                    .close_window(&self.ctx, title)
                    .await
                    .map_err(|e| format!("close failed: {}", e))?;
                Ok(format!("closed window '{}'", title))
            }

            ActionStrategy::OpenUrl { url } => {
                self.kernel
                    .desktop
                    .open_url(&self.ctx, url)
                    .await
                    .map_err(|e| format!("open URL failed: {}", e))?;
                Ok(format!("opened URL: {}", url))
            }

            ActionStrategy::LaunchApp { app } => {
                self.kernel
                    .desktop
                    .launch_app(&self.ctx, app)
                    .await
                    .map_err(|e| format!("launch failed: {}", e))?;
                Ok(format!("launched app '{}'", app))
            }

            ActionStrategy::MouseMove { x, y } => {
                self.kernel
                    .input
                    .mouse_move(&self.ctx, *x, *y)
                    .await
                    .map_err(|e| format!("mousemove failed: {}", e))?;
                Ok(format!("moved mouse to ({}, {})", x, y))
            }

            ActionStrategy::MouseClick { button } => {
                self.kernel
                    .input
                    .mouse_click(&self.ctx, button)
                    .await
                    .map_err(|e| format!("click failed: {}", e))?;
                Ok(format!("clicked button {}", button))
            }

            ActionStrategy::MouseMoveClick { x, y, button } => {
                self.kernel
                    .input
                    .mouse_move(&self.ctx, *x, *y)
                    .await
                    .map_err(|e| format!("mousemove failed: {}", e))?;
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                self.kernel
                    .input
                    .mouse_click(&self.ctx, button)
                    .await
                    .map_err(|e| format!("click failed: {}", e))?;
                Ok(format!("clicked at ({}, {})", x, y))
            }

            ActionStrategy::KeyboardType { text } => {
                self.kernel
                    .input
                    .keyboard_type(&self.ctx, text)
                    .await
                    .map_err(|e| format!("type failed: {}", e))?;
                Ok(format!("typed {} chars", text.len()))
            }

            ActionStrategy::KeyboardCombo { keys } => {
                let key_refs: Vec<&str> = keys.iter().map(|s| s.as_str()).collect();
                self.kernel
                    .input
                    .keyboard_combo(&self.ctx, &key_refs)
                    .await
                    .map_err(|e| format!("key combo failed: {}", e))?;
                Ok(format!("sent key combo: {:?}", keys))
            }

            ActionStrategy::ClipboardSet { text } => {
                self.kernel
                    .clipboard
                    .set_text(&self.ctx, text)
                    .await
                    .map_err(|e| format!("clipboard set failed: {}", e))?;
                Ok(format!("set clipboard ({} bytes)", text.len()))
            }

            ActionStrategy::ShellCommand { command } => {
                if command.is_empty() {
                    return Err("empty command".into());
                }
                let output = std::process::Command::new(&command[0])
                    .args(&command[1..])
                    .output()
                    .map_err(|e| format!("command failed: {}", e))?;
                let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if output.status.success() {
                    Ok(stdout)
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                    Err(format!("exit={:?}: {}", output.status.code(), stderr))
                }
            }

            ActionStrategy::WaitMs { duration_ms } => {
                tokio::time::sleep(tokio::time::Duration::from_millis(*duration_ms)).await;
                Ok(format!("waited {}ms", duration_ms))
            }

            ActionStrategy::Noop => Ok("no operation".into()),
        }
    }

    pub async fn execute_action(&self, action: &Action) -> ActionResult {
        let before = self.observe().await;
        tracing::info!(
            "OPERATOR: observe BEFORE — {} windows, focus={:?}, cursor=({},{}), clipboard={}",
            before.windows.len(),
            before.focused_window.as_ref().map(|w| &w.title),
            before.cursor_position.0,
            before.cursor_position.1,
            before.clipboard.as_deref().unwrap_or("(none)")
        );

        let strategies =
            StrategySelector::strategies_for(&action.capability_id, &action.params, &before);
        tracing::info!(
            "OPERATOR: strategies_for(\"{}\") = {} strategies",
            action.capability_id,
            strategies.len()
        );
        for (i, s) in strategies.iter().enumerate() {
            tracing::info!("  strategy[{}]: {:?}", i, s);
        }

        let mut last_error: Option<String> = None;
        let mut attempts = 0u32;
        let mut after = self.observe().await;
        let mut success = false;
        let mut strategy_used = "none".to_string();
        let mut verification = VerificationResult::Failed {
            reason: "no strategies attempted".into(),
        };

        for strategy in &strategies {
            attempts += 1;
            strategy_used = format!("{:?}", strategy)
                .split(' ')
                .next()
                .unwrap_or("unknown")
                .to_string();

            tracing::info!("OPERATOR: attempt {} — strategy={:?}", attempts, strategy);
            match self.execute_strategy(strategy).await {
                Ok(msg) => {
                    tracing::info!("  OSAL result: {}", msg);
                    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
                    after = self.observe().await;
                    tracing::info!("OPERATOR: observe AFTER — {} windows, focus={:?}, cursor=({},{}), clipboard={}",
                        after.windows.len(),
                        after.focused_window.as_ref().map(|w| &w.title),
                        after.cursor_position.0, after.cursor_position.1,
                        after.clipboard.as_deref().unwrap_or("(none)"));

                    verification = VerificationEngine::verify(
                        &action.capability_id,
                        &action.params,
                        &before,
                        &after,
                    );
                    tracing::info!("  verification: {:?}", verification);

                    if verification.is_success() {
                        success = true;
                        break;
                    }
                    last_error = Some("verification failed".into());
                }
                Err(e) => {
                    tracing::info!("  OSAL error: {}", e);
                    last_error = Some(e);
                    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                    after = self.observe().await;
                }
            }
        }

        ActionResult {
            capability_id: action.capability_id.clone(),
            success,
            strategy_used,
            attempts,
            before_state: before,
            after_state: after,
            verification,
            error: last_error,
        }
    }
}
