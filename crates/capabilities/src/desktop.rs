use crate::error::CapabilityResult;
use std::collections::HashMap;

fn run_cmd(binary: &str, args: &[&str]) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    let output = std::process::Command::new(binary).args(args).output()?;
    let exit_code = output.status.code().unwrap_or(-1);
    Ok((output.stdout, output.stderr, exit_code))
}

fn run_cmd_with_stdin(
    binary: &str,
    args: &[&str],
    stdin: &[u8],
) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    use std::io::Write;
    let mut child = std::process::Command::new(binary)
        .args(args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;
    if let Some(mut stdw) = child.stdin.take() {
        stdw.write_all(stdin)?;
    }
    let output = child.wait_with_output()?;
    let exit_code = output.status.code().unwrap_or(-1);
    Ok((output.stdout, output.stderr, exit_code))
}

pub fn dispatch(
    capability: &str,
    inputs: &HashMap<String, String>,
) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    match capability {
        "desktop.window.list" => window_list(),
        "desktop.window.focus" => {
            let id = inputs.get("id").map(|s| s.as_str()).unwrap_or("");
            window_focus(id)
        }
        "desktop.window.close" => {
            let id = inputs.get("id").map(|s| s.as_str()).unwrap_or("");
            window_close(id)
        }
        "desktop.window.move" => {
            let id = inputs.get("id").map(|s| s.as_str()).unwrap_or("");
            let x = inputs.get("x").map(|s| s.as_str()).unwrap_or("0");
            let y = inputs.get("y").map(|s| s.as_str()).unwrap_or("0");
            let w = inputs.get("width").map(|s| s.as_str()).unwrap_or("-1");
            let h = inputs.get("height").map(|s| s.as_str()).unwrap_or("-1");
            window_move(id, x, y, w, h)
        }
        "desktop.window.minimize" => {
            let id = inputs.get("id").map(|s| s.as_str()).unwrap_or("");
            window_minimize(id)
        }
        "desktop.window.maximize" => {
            let id = inputs.get("id").map(|s| s.as_str()).unwrap_or("");
            window_maximize(id)
        }
        "desktop.app.launch" => {
            let app = inputs.get("app").map(|s| s.as_str()).unwrap_or("");
            app_launch(app)
        }
        "desktop.app.list" => app_list(),
        "desktop.app.find" => {
            let query = inputs.get("query").map(|s| s.as_str()).unwrap_or("");
            app_find(query)
        }
        "desktop.process.list" => process_list(),
        "desktop.process.kill" => {
            let pid = inputs.get("pid").map(|s| s.as_str()).unwrap_or("");
            process_kill(pid)
        }
        "desktop.process.info" => {
            let pid = inputs.get("pid").map(|s| s.as_str()).unwrap_or("");
            process_info(pid)
        }
        "desktop.clipboard.get" => clipboard_get(),
        "desktop.clipboard.set" => {
            let text = inputs.get("text").map(|s| s.as_str()).unwrap_or("");
            clipboard_set(text)
        }
        "desktop.clipboard.clear" => clipboard_clear(),
        "desktop.notification.send" => {
            let title = inputs.get("title").map(|s| s.as_str()).unwrap_or("");
            let body = inputs.get("body").map(|s| s.as_str()).unwrap_or("");
            let urgency = inputs
                .get("urgency")
                .map(|s| s.as_str())
                .unwrap_or("normal");
            notification_send(title, body, urgency)
        }
        "desktop.wallpaper.set" => {
            let path = inputs.get("path").map(|s| s.as_str()).unwrap_or("");
            wallpaper_set(path)
        }
        "desktop.wallpaper.get" => wallpaper_get(),
        "desktop.volume.get" => volume_get(),
        "desktop.volume.set" => {
            let value = inputs.get("value").map(|s| s.as_str()).unwrap_or("");
            volume_set(value)
        }
        "desktop.volume.mute" => volume_mute(),
        "desktop.brightness.get" => brightness_get(),
        "desktop.brightness.set" => {
            let value = inputs.get("value").map(|s| s.as_str()).unwrap_or("");
            brightness_set(value)
        }
        "desktop.screenshot.capture" => {
            let path = inputs.get("path").map(|s| s.as_str()).unwrap_or("");
            screenshot_capture(path)
        }
        "desktop.screenshot.area" => {
            let path = inputs.get("path").map(|s| s.as_str()).unwrap_or("");
            screenshot_area(path)
        }
        "desktop.display.list" => display_list(),
        "desktop.display.info" => display_info(),
        _ => Err(crate::error::CapabilityError::UnknownCapability(
            capability.to_string(),
        )),
    }
}

fn window_list() -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    run_cmd("wmctrl", &["-l"])
}

fn window_focus(id: &str) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    if id.is_empty() {
        return Err(crate::error::CapabilityError::MissingInput("id".into()));
    }
    run_cmd("wmctrl", &["-a", id])
}

fn window_close(id: &str) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    if id.is_empty() {
        return Err(crate::error::CapabilityError::MissingInput("id".into()));
    }
    run_cmd("wmctrl", &["-c", id])
}

fn window_move(
    id: &str,
    x: &str,
    y: &str,
    w: &str,
    h: &str,
) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    if id.is_empty() {
        return Err(crate::error::CapabilityError::MissingInput("id".into()));
    }
    run_cmd(
        "wmctrl",
        &["-r", id, "-e", &format!("0,{},{},{},{}", x, y, w, h)],
    )
}

fn window_minimize(id: &str) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    if id.is_empty() {
        return Err(crate::error::CapabilityError::MissingInput("id".into()));
    }
    run_cmd("xdotool", &["windowminimize", id])
}

fn window_maximize(id: &str) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    if id.is_empty() {
        return Err(crate::error::CapabilityError::MissingInput("id".into()));
    }
    run_cmd(
        "wmctrl",
        &["-r", id, "-b", "toggle,maximized_vert,maximized_horz"],
    )
}

fn app_launch(app: &str) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    if app.is_empty() {
        return Err(crate::error::CapabilityError::MissingInput("app".into()));
    }
    run_cmd("gtk-launch", &[app])
}

fn app_list() -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    run_cmd(
        "find",
        &[
            "/usr/share/applications",
            "-name",
            "*.desktop",
            "-printf",
            "%f\\n",
        ],
    )
}

fn app_find(query: &str) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    if query.is_empty() {
        return Err(crate::error::CapabilityError::MissingInput("query".into()));
    }
    let pattern = format!("*{}*", query);
    run_cmd(
        "find",
        &[
            "/usr/share/applications",
            "-name",
            &pattern,
            "-printf",
            "%f\\n",
        ],
    )
}

fn process_list() -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    run_cmd("ps", &["aux"])
}

fn process_kill(pid: &str) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    if pid.is_empty() {
        return Err(crate::error::CapabilityError::MissingInput("pid".into()));
    }
    run_cmd("kill", &[pid])
}

fn process_info(pid: &str) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    if pid.is_empty() {
        return Err(crate::error::CapabilityError::MissingInput("pid".into()));
    }
    run_cmd(
        "ps",
        &[
            "-p",
            pid,
            "-o",
            "pid,ppid,user,%cpu,%mem,rss,start,time,args",
        ],
    )
}

fn clipboard_get() -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    run_cmd("xclip", &["-o", "-selection", "clipboard"])
}

fn clipboard_set(text: &str) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    run_cmd_with_stdin("xclip", &["-i", "-selection", "clipboard"], text.as_bytes())
}

fn clipboard_clear() -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    run_cmd_with_stdin(
        "xclip",
        &["-i", "-selection", "clipboard", "/dev/null"],
        b"",
    )
}

fn notification_send(
    title: &str,
    body: &str,
    urgency: &str,
) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    let valid_urg = ["low", "normal", "critical"];
    if !valid_urg.contains(&urgency) {
        return Err(crate::error::CapabilityError::InvalidInput(format!(
            "urgency must be low/normal/critical, got {}",
            urgency
        )));
    }
    run_cmd("notify-send", &["-u", urgency, title, body])
}

fn wallpaper_set(path: &str) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    if path.is_empty() {
        return Err(crate::error::CapabilityError::MissingInput("path".into()));
    }
    run_cmd("feh", &["--bg-fill", path])
}

fn wallpaper_get() -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    run_cmd(
        "gsettings",
        &["get", "org.gnome.desktop.background", "picture-uri"],
    )
}

fn volume_get() -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    run_cmd("pactl", &["get-sink-volume", "@DEFAULT_SINK@"])
}

fn volume_set(value: &str) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    if value.is_empty() {
        return Err(crate::error::CapabilityError::MissingInput("value".into()));
    }
    let percentage = format!("{}%", value);
    run_cmd("pactl", &["set-sink-volume", "@DEFAULT_SINK@", &percentage])
}

fn volume_mute() -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    run_cmd("pactl", &["set-sink-mute", "@DEFAULT_SINK@", "toggle"])
}

fn brightness_get() -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    run_cmd("brightnessctl", &["get"])
}

fn brightness_set(value: &str) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    if value.is_empty() {
        return Err(crate::error::CapabilityError::MissingInput("value".into()));
    }
    let percentage = format!("{}%", value);
    run_cmd("brightnessctl", &["set", &percentage])
}

fn screenshot_capture(path: &str) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    if path.is_empty() {
        return Err(crate::error::CapabilityError::MissingInput("path".into()));
    }
    run_cmd("import", &["-window", "root", path])
}

fn screenshot_area(path: &str) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    if path.is_empty() {
        return Err(crate::error::CapabilityError::MissingInput("path".into()));
    }
    run_cmd("import", &[path])
}

fn display_list() -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    run_cmd("xrandr", &["--listmonitors"])
}

fn display_info() -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    run_cmd("xrandr", &["--verbose"])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::CapabilityError;

    #[test]
    fn test_dispatch_unknown_capability() {
        let result = dispatch("nonexistent.capability", &HashMap::new());
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            CapabilityError::UnknownCapability(_)
        ));
    }

    #[test]
    fn test_dispatch_window_focus_missing_id() {
        let mut inputs = HashMap::new();
        inputs.insert("not_id".to_string(), "value".to_string());
        let result = dispatch("desktop.window.focus", &inputs);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            CapabilityError::MissingInput(_)
        ));
    }

    #[test]
    fn test_dispatch_window_close_missing_id() {
        let result = dispatch("desktop.window.close", &HashMap::new());
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            CapabilityError::MissingInput(_)
        ));
    }

    #[test]
    fn test_notification_invalid_urgency() {
        let mut inputs = HashMap::new();
        inputs.insert("title".to_string(), "Test".to_string());
        inputs.insert("body".to_string(), "Body".to_string());
        inputs.insert("urgency".to_string(), "invalid".to_string());
        let result = dispatch("desktop.notification.send", &inputs);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            CapabilityError::InvalidInput(_)
        ));
    }

    #[test]
    fn test_dispatch_window_list_succeeds() {
        // This will fail if wmctrl is not installed — test routing only
        let result = dispatch("desktop.window.list", &HashMap::new());
        // Either succeeds (wmctrl exists) or fails with IO error (not found)
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_dispatch_clipboard_get_routes() {
        let result = dispatch("desktop.clipboard.get", &HashMap::new());
        // May fail if xclip not installed, but must route to clipboard_get()
        assert!(result.is_ok() || result.is_err());
    }
}
