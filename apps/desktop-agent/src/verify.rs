//! Action outcome verification.
//!
//! Compares before/after state snapshots to determine if an action had
//! the expected effect. No application-specific logic — only generic
//! desktop state comparison primitives.

use std::collections::HashMap;

use perception_core::DesktopState;

/// Result of verifying an action's outcome.
#[derive(Debug, Clone)]
pub enum VerificationResult {
    Success,
    PartialSuccess { reason: String },
    Failed { reason: String },
}

impl VerificationResult {
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success)
    }
}

pub struct VerificationEngine;

impl VerificationEngine {
    /// Verify whether an action had the expected effect.
    ///
    /// Uses capability_id to select verification strategy, but only
    /// generic observation primitives — no application-specific checks.
    pub fn verify(
        capability_id: &str,
        params: &HashMap<String, String>,
        before: &DesktopState,
        after: &DesktopState,
    ) -> VerificationResult {
        match capability_id {
            "desktop.window.focus" => Self::verify_window_focus(before, after),
            "desktop.window.close" => Self::verify_window_closed(before, after),
            "desktop.app.launch" => Self::verify_window_count_increased(before, after),
            "input.mouse.move" => Self::verify_mouse_moved(params, before, after),
            "input.mouse.click" => VerificationResult::Success,
            "input.keyboard.type" => Self::verify_state_changed(before, after),
            "desktop.clipboard.set" => Self::verify_clipboard_set(params, before, after),
            _ => Self::verify_state_changed(before, after),
        }
    }

    fn verify_window_focus(_before: &DesktopState, after: &DesktopState) -> VerificationResult {
        if after.focused_window.is_some() {
            VerificationResult::Success
        } else {
            VerificationResult::PartialSuccess {
                reason: "no window appears focused".into(),
            }
        }
    }

    fn verify_window_closed(before: &DesktopState, after: &DesktopState) -> VerificationResult {
        if after.windows.len() < before.windows.len() {
            VerificationResult::Success
        } else {
            VerificationResult::Failed {
                reason: format!(
                    "window count did not decrease (was {}, now {})",
                    before.windows.len(),
                    after.windows.len()
                ),
            }
        }
    }

    fn verify_window_count_increased(
        before: &DesktopState,
        after: &DesktopState,
    ) -> VerificationResult {
        if after.windows.len() > before.windows.len() {
            VerificationResult::Success
        } else {
            VerificationResult::PartialSuccess {
                reason: "no new window appeared".into(),
            }
        }
    }

    fn verify_mouse_moved(
        params: &HashMap<String, String>,
        _before: &DesktopState,
        after: &DesktopState,
    ) -> VerificationResult {
        let target_x = params
            .get("x")
            .and_then(|v| v.parse::<i32>().ok())
            .unwrap_or(0);
        let target_y = params
            .get("y")
            .and_then(|v| v.parse::<i32>().ok())
            .unwrap_or(0);
        let dx = (after.cursor_position.0 - target_x).abs();
        let dy = (after.cursor_position.1 - target_y).abs();
        if dx < 10 && dy < 10 {
            VerificationResult::Success
        } else {
            VerificationResult::Failed {
                reason: format!(
                    "cursor at ({},{}), expected near ({},{})",
                    after.cursor_position.0, after.cursor_position.1, target_x, target_y
                ),
            }
        }
    }

    fn verify_clipboard_set(
        params: &HashMap<String, String>,
        _before: &DesktopState,
        after: &DesktopState,
    ) -> VerificationResult {
        let expected = params.get("text").map(|s| s.as_str()).unwrap_or("");
        if after.clipboard.as_deref() == Some(expected) {
            VerificationResult::Success
        } else if after.clipboard.is_some() {
            VerificationResult::PartialSuccess {
                reason: "clipboard content changed but does not match expected".into(),
            }
        } else {
            VerificationResult::Failed {
                reason: "clipboard is empty".into(),
            }
        }
    }

    /// Generic: any state change counts as success.
    fn verify_state_changed(before: &DesktopState, after: &DesktopState) -> VerificationResult {
        if after.windows.len() != before.windows.len()
            || after.focused_window.as_ref().map(|w| &w.window_id)
                != before.focused_window.as_ref().map(|w| &w.window_id)
            || after.cursor_position != before.cursor_position
            || after.clipboard != before.clipboard
        {
            VerificationResult::Success
        } else {
            VerificationResult::PartialSuccess {
                reason: "no observable change detected".into(),
            }
        }
    }
}
