use std::fmt;
use std::process::Command as StdCommand;

use async_trait::async_trait;
use osal_capabilities::{Capability, CapabilityContext};
use osal_core::{
    ClipboardProvider, DesktopError, DesktopProvider, InputDevice, InputError, Rect, WindowError,
    WindowInfo, WindowManager,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn detect_display_server() -> Result<String, DesktopError> {
    if let Ok(session) = std::env::var("XDG_SESSION_TYPE") {
        let lower = session.to_lowercase();
        if lower == "x11" {
            return Ok("x11".into());
        }
        if lower == "wayland" {
            return Err(DesktopError::NotAvailable(
                "Wayland desktop automation not yet supported".into(),
            ));
        }
    }
    if std::env::var("DISPLAY").is_ok() {
        return Ok("x11".into());
    }
    if std::env::var("WAYLAND_DISPLAY").is_ok() {
        return Err(DesktopError::NotAvailable(
            "Wayland desktop automation not yet supported".into(),
        ));
    }
    Err(DesktopError::NoDisplayServer)
}

async fn run_cmd(program: &str, args: &[&str]) -> Result<String, String> {
    let program = program.to_string();
    let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
    tokio::task::spawn_blocking(move || {
        let output = StdCommand::new(&program)
            .args(&args)
            .output()
            .map_err(|e| format!("failed to execute {}: {}", program, e))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("{} failed: {}", program, stderr.trim()));
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    })
    .await
    .map_err(|e| format!("spawn_blocking join: {}", e))?
}

async fn run_cmd_with_stdin(program: &str, args: &[&str], input: &str) -> Result<String, String> {
    let program = program.to_string();
    let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
    let input = input.to_string();
    tokio::task::spawn_blocking(move || {
        let mut child = StdCommand::new(&program)
            .args(&args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("failed to execute {}: {}", program, e))?;

        use std::io::Write;
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(input.as_bytes())
                .map_err(|e| format!("failed to write stdin to {}: {}", program, e))?;
        }

        let output = child
            .wait_with_output()
            .map_err(|e| format!("failed to wait for {}: {}", program, e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("{} failed: {}", program, stderr.trim()));
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    })
    .await
    .map_err(|e| format!("spawn_blocking join: {}", e))?
}

// ---------------------------------------------------------------------------
// LinuxWindowManager
// ---------------------------------------------------------------------------

pub struct LinuxWindowManager;

impl LinuxWindowManager {
    pub fn new() -> Self {
        Self
    }
}

impl fmt::Debug for LinuxWindowManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LinuxWindowManager").finish()
    }
}

#[async_trait]
impl WindowManager for LinuxWindowManager {
    async fn list_windows(&self, ctx: &CapabilityContext) -> Result<Vec<WindowInfo>, WindowError> {
        if !ctx.capabilities.check(&Capability::WindowList) {
            return Err(WindowError::PermissionDenied("WindowList denied".into()));
        }
        detect_display_server().map_err(|e| WindowError::NotAvailable(e.to_string()))?;

        let stdout = run_cmd("wmctrl", &["-l"])
            .await
            .map_err(|e| WindowError::Io(e))?;

        let mut windows = Vec::new();
        for line in stdout.lines() {
            let parts: Vec<&str> = line
                .splitn(4, |c: char| c.is_whitespace())
                .filter(|s| !s.is_empty())
                .collect();
            if parts.len() < 2 {
                continue;
            }
            let win_id = parts[0].to_string();
            let title = if parts.len() >= 4 {
                parts[3].to_string()
            } else {
                String::new()
            };

            let geometry = get_window_geometry(&win_id).await.ok();

            windows.push(WindowInfo {
                window_id: win_id,
                title,
                process: None,
                geometry,
                is_visible: true,
            });
        }

        Ok(windows)
    }

    async fn focused_window(
        &self,
        ctx: &CapabilityContext,
    ) -> Result<Option<WindowInfo>, WindowError> {
        if !ctx.capabilities.check(&Capability::WindowList) {
            return Err(WindowError::PermissionDenied("WindowList denied".into()));
        }
        detect_display_server().map_err(|e| WindowError::NotAvailable(e.to_string()))?;

        let stdout = run_cmd("xdotool", &["getactivewindow"])
            .await
            .map_err(|e| WindowError::Io(e))?;
        let id = stdout.trim();
        if id.is_empty() {
            return Ok(None);
        }

        let title_stdout = run_cmd("xdotool", &["getwindowname", id])
            .await
            .unwrap_or_default();
        let geometry = get_window_geometry(id).await.ok();

        Ok(Some(WindowInfo {
            window_id: id.to_string(),
            title: title_stdout.trim().to_string(),
            process: None,
            geometry,
            is_visible: true,
        }))
    }

    async fn focus_window(
        &self,
        ctx: &CapabilityContext,
        title: &str,
        _partial: bool,
    ) -> Result<(), WindowError> {
        if !ctx.capabilities.check(&Capability::WindowFocus) {
            return Err(WindowError::PermissionDenied("WindowFocus denied".into()));
        }
        detect_display_server().map_err(|e| WindowError::NotAvailable(e.to_string()))?;

        let win_id = find_window_id(title).await?;

        run_cmd("xdotool", &["windowactivate", &win_id])
            .await
            .map_err(|e| WindowError::Io(e))?;

        Ok(())
    }

    async fn close_window(&self, ctx: &CapabilityContext, title: &str) -> Result<(), WindowError> {
        if !ctx.capabilities.check(&Capability::WindowFocus) {
            return Err(WindowError::PermissionDenied("WindowFocus denied".into()));
        }
        detect_display_server().map_err(|e| WindowError::NotAvailable(e.to_string()))?;

        let win_id = find_window_id(title).await?;

        run_cmd("xdotool", &["windowclose", &win_id])
            .await
            .map_err(|e| WindowError::Io(e))?;

        Ok(())
    }

    async fn screen_dimensions(&self, ctx: &CapabilityContext) -> Result<(u32, u32), WindowError> {
        if !ctx.capabilities.check(&Capability::WindowList) {
            return Err(WindowError::PermissionDenied("WindowList denied".into()));
        }
        detect_display_server().map_err(|e| WindowError::NotAvailable(e.to_string()))?;

        let stdout = run_cmd("xdotool", &["getdisplaygeometry"])
            .await
            .map_err(|e| WindowError::Io(e))?;

        let parts: Vec<&str> = stdout.split_whitespace().collect();
        let w: u32 = parts.first().and_then(|s| s.parse().ok()).unwrap_or(1024);
        let h: u32 = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(768);

        Ok((w, h))
    }
}

async fn find_window_id(title: &str) -> Result<String, WindowError> {
    let stdout = run_cmd("xdotool", &["search", "--name", title])
        .await
        .map_err(|e| WindowError::Io(e))?;
    let id = stdout.lines().next().unwrap_or("").trim().to_string();
    if id.is_empty() {
        Err(WindowError::NotFound(title.to_string()))
    } else {
        Ok(id)
    }
}

async fn get_window_geometry(id: &str) -> Result<Rect, WindowError> {
    let stdout = run_cmd("xdotool", &["getwindowgeometry", id])
        .await
        .map_err(|e| WindowError::Io(e))?;
    let mut x = 0i32;
    let mut y = 0i32;
    let mut w = 0u32;
    let mut h = 0u32;
    for line in stdout.lines() {
        if let Some(pos) = line.strip_prefix("  Position: ") {
            let coords: Vec<&str> = pos.split(',').collect();
            x = coords
                .first()
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(0);
            y = coords
                .get(1)
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(0);
        } else if let Some(geo) = line.strip_prefix("  Geometry: ") {
            let dims: Vec<&str> = geo.split('x').collect();
            w = dims.first().and_then(|s| s.parse().ok()).unwrap_or(0);
            h = dims.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
        }
    }
    Ok(Rect {
        x,
        y,
        width: w,
        height: h,
    })
}

// ---------------------------------------------------------------------------
// LinuxInputDevice
// ---------------------------------------------------------------------------

pub struct LinuxInputDevice;

impl LinuxInputDevice {
    pub fn new() -> Self {
        Self
    }
}

impl fmt::Debug for LinuxInputDevice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LinuxInputDevice").finish()
    }
}

#[async_trait]
impl InputDevice for LinuxInputDevice {
    async fn mouse_move(&self, ctx: &CapabilityContext, x: i32, y: i32) -> Result<(), InputError> {
        if !ctx.capabilities.check(&Capability::InputMouse) {
            return Err(InputError::PermissionDenied("InputMouse denied".into()));
        }
        detect_display_server().map_err(|e| InputError::NotAvailable(e.to_string()))?;

        run_cmd("xdotool", &["mousemove", &x.to_string(), &y.to_string()])
            .await
            .map_err(|e| InputError::Io(e))?;

        Ok(())
    }

    async fn mouse_click(&self, ctx: &CapabilityContext, button: &str) -> Result<(), InputError> {
        if !ctx.capabilities.check(&Capability::InputMouse) {
            return Err(InputError::PermissionDenied("InputMouse denied".into()));
        }
        detect_display_server().map_err(|e| InputError::NotAvailable(e.to_string()))?;

        let btn = match button {
            "left" | "1" => "1",
            "middle" | "2" => "2",
            "right" | "3" => "3",
            _ => "1",
        };

        run_cmd("xdotool", &["click", btn])
            .await
            .map_err(|e| InputError::Io(e))?;

        Ok(())
    }

    async fn keyboard_type(&self, ctx: &CapabilityContext, text: &str) -> Result<(), InputError> {
        if !ctx.capabilities.check(&Capability::InputKeyboard) {
            return Err(InputError::PermissionDenied("InputKeyboard denied".into()));
        }
        detect_display_server().map_err(|e| InputError::NotAvailable(e.to_string()))?;

        run_cmd("xdotool", &["type", "--", text])
            .await
            .map_err(|e| InputError::Io(e))?;

        Ok(())
    }

    async fn keyboard_combo(
        &self,
        ctx: &CapabilityContext,
        keys: &[&str],
    ) -> Result<(), InputError> {
        if !ctx.capabilities.check(&Capability::InputKeyboard) {
            return Err(InputError::PermissionDenied("InputKeyboard denied".into()));
        }
        detect_display_server().map_err(|e| InputError::NotAvailable(e.to_string()))?;

        let mut args = vec!["key".to_string()];
        args.push(keys.join("+"));
        let args_str: Vec<&str> = args.iter().map(|s| s.as_str()).collect();

        run_cmd("xdotool", &args_str)
            .await
            .map_err(|e| InputError::Io(e))?;

        Ok(())
    }

    async fn cursor_position(&self, ctx: &CapabilityContext) -> Result<(i32, i32), InputError> {
        if !ctx.capabilities.check(&Capability::InputMouse) {
            return Err(InputError::PermissionDenied("InputMouse denied".into()));
        }
        detect_display_server().map_err(|e| InputError::NotAvailable(e.to_string()))?;

        let stdout = run_cmd("xdotool", &["getmouselocation"])
            .await
            .map_err(|e| InputError::Io(e))?;

        let mut x = 0i32;
        let mut y = 0i32;
        for part in stdout.split_whitespace() {
            if let Some(val) = part.strip_prefix("x:") {
                x = val.parse().unwrap_or(0);
            } else if let Some(val) = part.strip_prefix("y:") {
                y = val.parse().unwrap_or(0);
            }
        }

        Ok((x, y))
    }
}

// ---------------------------------------------------------------------------
// LinuxClipboardProvider
// ---------------------------------------------------------------------------

pub struct LinuxClipboardProvider;

impl LinuxClipboardProvider {
    pub fn new() -> Self {
        Self
    }
}

impl fmt::Debug for LinuxClipboardProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LinuxClipboardProvider").finish()
    }
}

#[async_trait]
impl ClipboardProvider for LinuxClipboardProvider {
    async fn get_text(&self, ctx: &CapabilityContext) -> Result<Option<String>, DesktopError> {
        if !ctx.capabilities.check(&Capability::ClipboardRead) {
            return Err(DesktopError::PermissionDenied(
                "ClipboardRead denied".into(),
            ));
        }
        detect_display_server()?;

        let output = run_cmd("xclip", &["-o", "-selection", "clipboard"])
            .await
            .map_err(|e| DesktopError::Io(e))?;

        if output.is_empty() {
            Ok(None)
        } else {
            Ok(Some(output.trim_end().to_string()))
        }
    }

    async fn set_text(&self, ctx: &CapabilityContext, text: &str) -> Result<(), DesktopError> {
        if !ctx.capabilities.check(&Capability::ClipboardWrite) {
            return Err(DesktopError::PermissionDenied(
                "ClipboardWrite denied".into(),
            ));
        }
        detect_display_server()?;

        run_cmd_with_stdin("xclip", &["-selection", "clipboard"], text)
            .await
            .map_err(|e| DesktopError::Io(e))?;

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// LinuxDesktopProvider
// ---------------------------------------------------------------------------

pub struct LinuxDesktopProvider;

impl LinuxDesktopProvider {
    pub fn new() -> Self {
        Self
    }
}

impl fmt::Debug for LinuxDesktopProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LinuxDesktopProvider").finish()
    }
}

#[async_trait]
impl DesktopProvider for LinuxDesktopProvider {
    async fn open_url(&self, ctx: &CapabilityContext, url: &str) -> Result<(), DesktopError> {
        if !ctx.capabilities.check(&Capability::DesktopOpenUrl) {
            return Err(DesktopError::PermissionDenied(
                "DesktopOpenUrl denied".into(),
            ));
        }
        detect_display_server()?;

        run_cmd("xdg-open", &[url])
            .await
            .map_err(|e| DesktopError::LaunchFailed(e))?;

        Ok(())
    }

    async fn launch_app(&self, ctx: &CapabilityContext, app: &str) -> Result<(), DesktopError> {
        if !ctx.capabilities.check(&Capability::DesktopLaunchApp) {
            return Err(DesktopError::PermissionDenied(
                "DesktopLaunchApp denied".into(),
            ));
        }
        detect_display_server()?;

        let result = run_cmd("gtk-launch", &[app]).await;
        match result {
            Ok(_) => Ok(()),
            Err(e) => {
                let fallback = run_cmd("xdg-open", &[app]).await;
                fallback.map_err(|fe| {
                    DesktopError::LaunchFailed(format!("gtk-launch: {}, xdg-open: {}", e, fe))
                })?;
                Ok(())
            }
        }
    }

    async fn take_screenshot(
        &self,
        ctx: &CapabilityContext,
        path: &str,
    ) -> Result<(), DesktopError> {
        if !ctx.capabilities.check(&Capability::ScreenCapture) {
            return Err(DesktopError::PermissionDenied(
                "ScreenCapture denied".into(),
            ));
        }
        detect_display_server()?;

        run_cmd("import", &["-window", "root", path])
            .await
            .map_err(|e| DesktopError::Io(e))?;

        Ok(())
    }

    fn display_server(&self) -> Result<String, DesktopError> {
        detect_display_server()
    }
}
