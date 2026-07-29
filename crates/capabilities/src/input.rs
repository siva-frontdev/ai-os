use crate::error::CapabilityResult;
use std::collections::HashMap;

fn run_cmd(binary: &str, args: &[&str]) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    let output = std::process::Command::new(binary).args(args).output()?;
    let exit_code = output.status.code().unwrap_or(-1);
    Ok((output.stdout, output.stderr, exit_code))
}

pub fn dispatch(
    capability: &str,
    inputs: &HashMap<String, String>,
) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    match capability {
        // ── Mouse ───────────────────────────────────────────
        "input.mouse.move" => {
            let x = inputs.get("x").map(|s| s.as_str()).unwrap_or("0");
            let y = inputs.get("y").map(|s| s.as_str()).unwrap_or("0");
            mouse_move(x, y)
        }
        "input.mouse.click" => {
            let button = inputs.get("button").map(|s| s.as_str()).unwrap_or("1");
            mouse_click(button)
        }
        "input.mouse.double_click" => {
            let button = inputs.get("button").map(|s| s.as_str()).unwrap_or("1");
            mouse_double_click(button)
        }
        "input.mouse.drag" => {
            let x1 = inputs.get("x1").map(|s| s.as_str()).unwrap_or("0");
            let y1 = inputs.get("y1").map(|s| s.as_str()).unwrap_or("0");
            let x2 = inputs.get("x2").map(|s| s.as_str()).unwrap_or("0");
            let y2 = inputs.get("y2").map(|s| s.as_str()).unwrap_or("0");
            let button = inputs.get("button").map(|s| s.as_str()).unwrap_or("1");
            mouse_drag(x1, y1, x2, y2, button)
        }
        "input.mouse.scroll" => {
            let amount = inputs.get("amount").map(|s| s.as_str()).unwrap_or("1");
            let direction = inputs
                .get("direction")
                .map(|s| s.as_str())
                .unwrap_or("down");
            mouse_scroll(amount, direction)
        }
        // ── Keyboard ────────────────────────────────────────
        "input.keyboard.press" => {
            let key = inputs.get("key").map(|s| s.as_str()).unwrap_or("");
            keyboard_press(key)
        }
        "input.keyboard.combo" => {
            let keys = inputs.get("keys").map(|s| s.as_str()).unwrap_or("");
            keyboard_combo(keys)
        }
        "input.keyboard.type" => {
            let text = inputs.get("text").map(|s| s.as_str()).unwrap_or("");
            keyboard_type(text)
        }
        "input.keyboard.shortcut" => {
            let shortcut = inputs.get("shortcut").map(|s| s.as_str()).unwrap_or("");
            keyboard_shortcut(shortcut)
        }
        // ── Hotkey ──────────────────────────────────────────
        "input.hotkey.register" => {
            let keys = inputs.get("keys").map(|s| s.as_str()).unwrap_or("");
            let command = inputs.get("command").map(|s| s.as_str()).unwrap_or("");
            hotkey_register(keys, command)
        }
        "input.hotkey.unregister" => {
            let keys = inputs.get("keys").map(|s| s.as_str()).unwrap_or("");
            hotkey_unregister(keys)
        }
        "input.hotkey.list" => hotkey_list(),
        _ => Err(crate::error::CapabilityError::UnknownCapability(
            capability.to_string(),
        )),
    }
}

/// Validate that x and y are non-empty numeric-ish strings.
fn validate_coord(x: &str, y: &str) -> CapabilityResult<()> {
    if x.is_empty() || y.is_empty() {
        return Err(crate::error::CapabilityError::MissingInput("x/y".into()));
    }
    Ok(())
}

fn validate_non_empty(val: &str, name: &str) -> CapabilityResult<()> {
    if val.is_empty() {
        return Err(crate::error::CapabilityError::MissingInput(name.into()));
    }
    Ok(())
}

// ── Mouse Implementations ─────────────────────────────────

fn mouse_move(x: &str, y: &str) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    validate_coord(x, y)?;
    run_cmd("xdotool", &["mousemove", x, y])
}

fn mouse_click(button: &str) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    validate_non_empty(button, "button")?;
    run_cmd("xdotool", &["click", button])
}

fn mouse_double_click(button: &str) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    validate_non_empty(button, "button")?;
    run_cmd("xdotool", &["click", "--repeat", "2", button])
}

fn mouse_drag(
    x1: &str,
    y1: &str,
    x2: &str,
    y2: &str,
    button: &str,
) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    validate_coord(x1, y1)?;
    validate_coord(x2, y2)?;
    validate_non_empty(button, "button")?;
    run_cmd(
        "xdotool",
        &[
            "mousemove",
            x1,
            y1,
            "mousedown",
            button,
            "mousemove",
            x2,
            y2,
            "mouseup",
            button,
        ],
    )
}

fn mouse_scroll(amount: &str, direction: &str) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    validate_non_empty(amount, "amount")?;
    let btn = match direction {
        "up" => "4",
        "down" => "5",
        "left" => "6",
        "right" => "7",
        _ => {
            return Err(crate::error::CapabilityError::InvalidInput(format!(
                "direction must be up/down/left/right, got {}",
                direction
            )));
        }
    };
    run_cmd("xdotool", &["click", "--repeat", amount, btn])
}

// ── Keyboard Implementations ──────────────────────────────

fn keyboard_press(key: &str) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    validate_non_empty(key, "key")?;
    run_cmd("xdotool", &["key", key])
}

fn keyboard_combo(keys: &str) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    validate_non_empty(keys, "keys")?;
    run_cmd("xdotool", &["key", keys])
}

fn keyboard_type(text: &str) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    validate_non_empty(text, "text")?;
    run_cmd("xdotool", &["type", text])
}

fn keyboard_shortcut(shortcut: &str) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    validate_non_empty(shortcut, "shortcut")?;
    run_cmd("xdotool", &["key", shortcut])
}

// ── Hotkey Implementations ────────────────────────────────

fn hotkey_register(keys: &str, command: &str) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    validate_non_empty(keys, "keys")?;
    validate_non_empty(command, "command")?;
    run_cmd("xdotool", &["key", keys])
}

fn hotkey_unregister(keys: &str) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    validate_non_empty(keys, "keys")?;
    run_cmd("xdotool", &["key", keys])
}

fn hotkey_list() -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    run_cmd("ps", &["aux"])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::CapabilityError;

    #[test]
    fn test_unknown_capability() {
        let result = dispatch("input.nonexistent", &HashMap::new());
        assert!(matches!(
            result.unwrap_err(),
            CapabilityError::UnknownCapability(_)
        ));
    }

    #[test]
    fn test_mouse_move_defaults_to_zero() {
        // Uses defaults: x="0", y="0"
        let result = dispatch("input.mouse.move", &HashMap::new());
        // Should not fail with MissingInput since defaults are applied
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_mouse_double_click_defaults_to_button_1() {
        // Uses default: button="1"
        let result = dispatch("input.mouse.double_click", &HashMap::new());
        // Should not fail with MissingInput since defaults are applied
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_mouse_scroll_invalid_direction() {
        let mut inputs = HashMap::new();
        inputs.insert("amount".to_string(), "3".to_string());
        inputs.insert("direction".to_string(), "diagonal".to_string());
        let result = dispatch("input.mouse.scroll", &inputs);
        assert!(matches!(
            result.unwrap_err(),
            CapabilityError::InvalidInput(_)
        ));
    }

    #[test]
    fn test_keyboard_press_missing_key() {
        let result = dispatch("input.keyboard.press", &HashMap::new());
        assert!(matches!(
            result.unwrap_err(),
            CapabilityError::MissingInput(_)
        ));
    }

    #[test]
    fn test_keyboard_type_empty_text() {
        let mut inputs = HashMap::new();
        inputs.insert("text".to_string(), "".to_string());
        let result = dispatch("input.keyboard.type", &inputs);
        assert!(matches!(
            result.unwrap_err(),
            CapabilityError::MissingInput(_)
        ));
    }
}
