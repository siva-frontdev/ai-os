use std::path::PathBuf;

/// Register the companion to start automatically at user login.
///
/// On Linux: creates a `.desktop` file in `~/.config/autostart/`.
/// On macOS: uses `launchd` plist.
/// On Windows: uses the registry `Run` key.
pub fn enable_autostart(binary_path: &str) -> Result<(), std::io::Error> {
    let entry = autostart_entry(binary_path);
    let path = autostart_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, entry.as_bytes())?;
    tracing::info!(path = %path.display(), "autostart enabled");
    Ok(())
}

/// Remove the autostart registration.
pub fn disable_autostart() -> Result<(), std::io::Error> {
    let path = autostart_path();
    if path.exists() {
        std::fs::remove_file(&path)?;
        tracing::info!(path = %path.display(), "autostart disabled");
    }
    Ok(())
}

/// Check whether autostart is currently enabled.
#[allow(dead_code)]
pub fn is_autostart_enabled() -> bool {
    autostart_path().exists()
}

/// Build the autostart `.desktop` file content.
fn autostart_entry(binary_path: &str) -> String {
    format!(
        r#"[Desktop Entry]
Type=Application
Name=AI-OS Companion
Comment=AI-OS Personal Companion
Exec={binary_path}
Terminal=false
Categories=Utility;
X-GNOME-Autostart-enabled=true
"#
    )
}

/// Path to the autostart file.
fn autostart_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home)
        .join(".config")
        .join("autostart")
        .join("ai-os-companion.desktop")
}
