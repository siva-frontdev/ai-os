#![forbid(unsafe_code)]

pub mod browser;
pub mod desktop;
pub mod development;
pub mod error;
pub mod factory;
pub mod fs;
pub mod input;

pub use error::{CapabilityError, CapabilityResult};
pub use factory::{capability_binding, CapabilitiesRunner, CapabilitiesRunnerFactory};

use execution_core::{ExecutionCapability, ExecutionResult, ToolRegistry};
use std::collections::HashMap;

pub fn all_capabilities() -> Vec<(
    &'static str,
    &'static str,
    &'static str,
    Vec<(&'static str, &'static str)>,
    Vec<(&'static str, &'static str)>,
)> {
    vec![
        (
            "desktop.window.list",
            "List Windows",
            "List all open windows",
            vec![],
            vec![("output", "string")],
        ),
        (
            "desktop.window.focus",
            "Focus Window",
            "Bring a window to focus",
            vec![("id", "string")],
            vec![],
        ),
        (
            "desktop.window.close",
            "Close Window",
            "Close a window",
            vec![("id", "string")],
            vec![],
        ),
        (
            "desktop.window.move",
            "Move Window",
            "Move and resize a window",
            vec![
                ("id", "string"),
                ("x", "string"),
                ("y", "string"),
                ("width", "string"),
                ("height", "string"),
            ],
            vec![],
        ),
        (
            "desktop.window.minimize",
            "Minimize Window",
            "Minimize a window",
            vec![("id", "string")],
            vec![],
        ),
        (
            "desktop.window.maximize",
            "Maximize Window",
            "Toggle window maximized state",
            vec![("id", "string")],
            vec![],
        ),
        (
            "desktop.app.launch",
            "Launch App",
            "Launch an application by desktop file ID",
            vec![("app", "string")],
            vec![],
        ),
        (
            "desktop.app.list",
            "List Apps",
            "List installed applications",
            vec![],
            vec![("output", "string")],
        ),
        (
            "desktop.app.find",
            "Find App",
            "Search for installed applications",
            vec![("query", "string")],
            vec![("output", "string")],
        ),
        (
            "desktop.process.list",
            "List Processes",
            "List all running processes",
            vec![],
            vec![("output", "string")],
        ),
        (
            "desktop.process.kill",
            "Kill Process",
            "Kill a process by PID",
            vec![("pid", "string")],
            vec![],
        ),
        (
            "desktop.process.info",
            "Process Info",
            "Get detailed process information",
            vec![("pid", "string")],
            vec![("output", "string")],
        ),
        (
            "desktop.clipboard.get",
            "Get Clipboard",
            "Read text from clipboard",
            vec![],
            vec![("text", "string")],
        ),
        (
            "desktop.clipboard.set",
            "Set Clipboard",
            "Write text to clipboard",
            vec![("text", "string")],
            vec![],
        ),
        (
            "desktop.clipboard.clear",
            "Clear Clipboard",
            "Clear the clipboard",
            vec![],
            vec![],
        ),
        (
            "desktop.notification.send",
            "Send Notification",
            "Send a desktop notification",
            vec![
                ("title", "string"),
                ("body", "string"),
                ("urgency", "string"),
            ],
            vec![],
        ),
        (
            "desktop.wallpaper.set",
            "Set Wallpaper",
            "Set desktop wallpaper from image file",
            vec![("path", "string")],
            vec![],
        ),
        (
            "desktop.wallpaper.get",
            "Get Wallpaper",
            "Get current wallpaper path",
            vec![],
            vec![("path", "string")],
        ),
        (
            "desktop.volume.get",
            "Get Volume",
            "Get current audio volume level",
            vec![],
            vec![("value", "string")],
        ),
        (
            "desktop.volume.set",
            "Set Volume",
            "Set audio volume level (0-100)",
            vec![("value", "string")],
            vec![],
        ),
        (
            "desktop.volume.mute",
            "Mute Volume",
            "Toggle audio mute",
            vec![],
            vec![],
        ),
        (
            "desktop.brightness.get",
            "Get Brightness",
            "Get current display brightness",
            vec![],
            vec![("value", "string")],
        ),
        (
            "desktop.brightness.set",
            "Set Brightness",
            "Set display brightness (0-100)",
            vec![("value", "string")],
            vec![],
        ),
        (
            "desktop.screenshot.capture",
            "Capture Screenshot",
            "Capture full screen screenshot",
            vec![("path", "string")],
            vec![("path", "string")],
        ),
        (
            "desktop.screenshot.area",
            "Area Screenshot",
            "Capture a selected area screenshot",
            vec![("path", "string")],
            vec![("path", "string")],
        ),
        (
            "desktop.display.list",
            "List Displays",
            "List connected monitors",
            vec![],
            vec![("output", "string")],
        ),
        (
            "desktop.display.info",
            "Display Info",
            "Get detailed display information",
            vec![],
            vec![("output", "string")],
        ),
        // ── Input Automation ─────────────────────────────────
        (
            "input.mouse.move",
            "Move Mouse",
            "Move mouse cursor to coordinates",
            vec![("x", "string"), ("y", "string")],
            vec![],
        ),
        (
            "input.mouse.click",
            "Click Mouse",
            "Click a mouse button",
            vec![("button", "string")],
            vec![],
        ),
        (
            "input.mouse.double_click",
            "Double Click",
            "Double click a mouse button",
            vec![("button", "string")],
            vec![],
        ),
        (
            "input.mouse.drag",
            "Drag Mouse",
            "Drag from one point to another",
            vec![
                ("x1", "string"),
                ("y1", "string"),
                ("x2", "string"),
                ("y2", "string"),
                ("button", "string"),
            ],
            vec![],
        ),
        (
            "input.mouse.scroll",
            "Scroll Mouse",
            "Scroll in a direction",
            vec![("amount", "string"), ("direction", "string")],
            vec![],
        ),
        (
            "input.keyboard.press",
            "Press Key",
            "Press a single key",
            vec![("key", "string")],
            vec![],
        ),
        (
            "input.keyboard.combo",
            "Key Combo",
            "Press key combination (e.g. ctrl+c)",
            vec![("keys", "string")],
            vec![],
        ),
        (
            "input.keyboard.type",
            "Type Text",
            "Type text as keystrokes",
            vec![("text", "string")],
            vec![],
        ),
        (
            "input.keyboard.shortcut",
            "Shortcut",
            "Execute a keyboard shortcut",
            vec![("shortcut", "string")],
            vec![],
        ),
        (
            "input.hotkey.register",
            "Register Hotkey",
            "Register a global hotkey",
            vec![("keys", "string"), ("command", "string")],
            vec![],
        ),
        (
            "input.hotkey.unregister",
            "Unregister Hotkey",
            "Unregister a global hotkey",
            vec![("keys", "string")],
            vec![],
        ),
        (
            "input.hotkey.list",
            "List Hotkeys",
            "List registered global hotkeys",
            vec![],
            vec![("output", "string")],
        ),
        // ── Browser Automation (Phase 3) ──────────────────────
        (
            "browser.open",
            "Open Browser",
            "Open a browser window",
            vec![("browser", "string")],
            vec![],
        ),
        (
            "browser.open_url",
            "Open URL",
            "Open a URL in the browser",
            vec![("browser", "string"), ("url", "string")],
            vec![],
        ),
        (
            "browser.switch_tab",
            "Switch Tab",
            "Switch browser tab (next/prev/first/last)",
            vec![("direction", "string")],
            vec![],
        ),
        (
            "browser.close_tab",
            "Close Tab",
            "Close current browser tab",
            vec![],
            vec![],
        ),
        (
            "browser.read_dom",
            "Read DOM",
            "Read DOM content matching a CSS selector",
            vec![("selector", "string")],
            vec![("output", "string")],
        ),
        (
            "browser.click_element",
            "Click Element",
            "Click an element matched by CSS selector",
            vec![("selector", "string")],
            vec![],
        ),
        (
            "browser.fill_form",
            "Fill Form",
            "Fill a form field matched by CSS selector",
            vec![("selector", "string"), ("value", "string")],
            vec![],
        ),
        (
            "browser.upload_file",
            "Upload File",
            "Upload a file through the browser file picker",
            vec![("path", "string")],
            vec![],
        ),
        (
            "browser.download_file",
            "Download File",
            "Download a file via curl",
            vec![("url", "string"), ("dest", "string")],
            vec![],
        ),
        (
            "browser.wait_for_element",
            "Wait For Element",
            "Wait for element to appear in DOM",
            vec![("selector", "string"), ("timeout", "string")],
            vec![("output", "string")],
        ),
        (
            "browser.take_screenshot",
            "Take Screenshot",
            "Take a screenshot of the browser",
            vec![("path", "string")],
            vec![],
        ),
        (
            "browser.execute_javascript",
            "Execute JavaScript",
            "Execute JavaScript in the browser",
            vec![("code", "string")],
            vec![("output", "string")],
        ),
        // ── File System (Phase 4) ────────────────────────────
        (
            "fs.file.read",
            "Read File",
            "Read file contents as text",
            vec![("path", "string")],
            vec![("output", "string")],
        ),
        (
            "fs.file.write",
            "Write File",
            "Write text to a file",
            vec![("path", "string"), ("content", "string")],
            vec![],
        ),
        (
            "fs.file.copy",
            "Copy File",
            "Copy file or directory",
            vec![("source", "string"), ("destination", "string")],
            vec![],
        ),
        (
            "fs.file.move",
            "Move File",
            "Move/rename file or directory",
            vec![("source", "string"), ("destination", "string")],
            vec![],
        ),
        (
            "fs.file.delete",
            "Delete File",
            "Delete file or directory",
            vec![("path", "string")],
            vec![],
        ),
        (
            "fs.file.append",
            "Append File",
            "Append text to a file",
            vec![("path", "string"), ("content", "string")],
            vec![],
        ),
        (
            "fs.file.touch",
            "Touch File",
            "Create empty file or update timestamp",
            vec![("path", "string")],
            vec![],
        ),
        (
            "fs.file.stat",
            "File Stat",
            "Get file metadata",
            vec![("path", "string")],
            vec![("output", "string")],
        ),
        (
            "fs.dir.list",
            "List Directory",
            "List directory contents",
            vec![("path", "string")],
            vec![("output", "string")],
        ),
        (
            "fs.dir.create",
            "Create Directory",
            "Create directory (recursive)",
            vec![("path", "string")],
            vec![],
        ),
        (
            "fs.dir.delete",
            "Delete Directory",
            "Remove directory recursively",
            vec![("path", "string")],
            vec![],
        ),
        (
            "fs.dir.tree",
            "Directory Tree",
            "Show recursive directory tree",
            vec![("path", "string")],
            vec![("output", "string")],
        ),
        (
            "fs.search.name",
            "Search by Name",
            "Find files matching a glob pattern",
            vec![("root", "string"), ("pattern", "string")],
            vec![("output", "string")],
        ),
        (
            "fs.search.content",
            "Search by Content",
            "Find files containing text",
            vec![("root", "string"), ("text", "string")],
            vec![("output", "string")],
        ),
        (
            "fs.archive.create",
            "Create Archive",
            "Create archive (zip/tar.gz/tar.bz2/tar.xz/7z)",
            vec![("archive", "string"), ("sources", "string")],
            vec![],
        ),
        (
            "fs.archive.extract",
            "Extract Archive",
            "Extract archive to destination",
            vec![("archive", "string"), ("destination", "string")],
            vec![],
        ),
        (
            "fs.watch",
            "Watch Path",
            "Watch file or directory for changes",
            vec![("path", "string")],
            vec![],
        ),
        // ── Development: Git (Phase 5) ──────────────────────
        (
            "development.git.health",
            "Git Health",
            "Check git availability",
            vec![],
            vec![],
        ),
        (
            "development.git.clone",
            "Git Clone",
            "Clone a repository",
            vec![("url", "string"), ("directory", "string")],
            vec![],
        ),
        (
            "development.git.init",
            "Git Init",
            "Initialize a repository",
            vec![("directory", "string")],
            vec![],
        ),
        (
            "development.git.add",
            "Git Add",
            "Stage changes",
            vec![("path", "string")],
            vec![],
        ),
        (
            "development.git.commit",
            "Git Commit",
            "Commit staged changes",
            vec![("message", "string")],
            vec![],
        ),
        (
            "development.git.push",
            "Git Push",
            "Push commits to remote",
            vec![("remote", "string"), ("branch", "string")],
            vec![],
        ),
        (
            "development.git.pull",
            "Git Pull",
            "Pull from remote",
            vec![("remote", "string"), ("branch", "string")],
            vec![],
        ),
        (
            "development.git.checkout",
            "Git Checkout",
            "Switch branch",
            vec![("branch", "string"), ("create", "string")],
            vec![],
        ),
        (
            "development.git.branch",
            "Git Branch",
            "Manage branches",
            vec![("action", "string"), ("name", "string")],
            vec![],
        ),
        (
            "development.git.status",
            "Git Status",
            "Show working tree status",
            vec![],
            vec![("output", "string")],
        ),
        (
            "development.git.log",
            "Git Log",
            "Show commit log",
            vec![("max_count", "string")],
            vec![("output", "string")],
        ),
        (
            "development.git.diff",
            "Git Diff",
            "Show changes",
            vec![("staged", "string")],
            vec![("output", "string")],
        ),
        (
            "development.git.stash",
            "Git Stash",
            "Manage stash",
            vec![("action", "string")],
            vec![],
        ),
        (
            "development.git.tag",
            "Git Tag",
            "Create a tag",
            vec![("name", "string"), ("message", "string")],
            vec![],
        ),
        (
            "development.git.merge",
            "Git Merge",
            "Merge a branch",
            vec![("branch", "string")],
            vec![],
        ),
        (
            "development.git.rebase",
            "Git Rebase",
            "Rebase onto a branch",
            vec![("branch", "string")],
            vec![],
        ),
        (
            "development.git.reset",
            "Git Reset",
            "Reset HEAD",
            vec![("target", "string"), ("mode", "string")],
            vec![],
        ),
        (
            "development.git.fetch",
            "Git Fetch",
            "Fetch from remote",
            vec![("remote", "string")],
            vec![],
        ),
        (
            "development.git.remote",
            "Git Remote",
            "List remotes",
            vec![],
            vec![("output", "string")],
        ),
        (
            "development.git.config",
            "Git Config",
            "Set git config",
            vec![("key", "string"), ("value", "string")],
            vec![],
        ),
        (
            "development.git.clean",
            "Git Clean",
            "Clean untracked files",
            vec![],
            vec![],
        ),
        // ── Development: Terminal ────────────────────────────
        (
            "development.terminal.health",
            "Terminal Health",
            "Check shell availability",
            vec![],
            vec![],
        ),
        (
            "development.terminal.execute",
            "Terminal Execute",
            "Execute a shell command",
            vec![
                ("command", "string"),
                ("working_directory", "string"),
                ("timeout", "string"),
            ],
            vec![("output", "string")],
        ),
        (
            "development.terminal.stream_output",
            "Terminal Stream",
            "Stream command output",
            vec![("command", "string"), ("working_directory", "string")],
            vec![("output", "string")],
        ),
        // ── Development: SSH ─────────────────────────────────
        (
            "development.ssh.health",
            "SSH Health",
            "Check ssh availability",
            vec![],
            vec![],
        ),
        (
            "development.ssh.execute",
            "SSH Execute",
            "Run command on remote host",
            vec![
                ("host", "string"),
                ("command", "string"),
                ("port", "string"),
                ("user", "string"),
            ],
            vec![("output", "string")],
        ),
        (
            "development.ssh.connect",
            "SSH Connect",
            "Connect to remote host",
            vec![("host", "string"), ("port", "string"), ("user", "string")],
            vec![],
        ),
        (
            "development.ssh.upload",
            "SSH Upload",
            "Upload file via scp",
            vec![
                ("host", "string"),
                ("source", "string"),
                ("destination", "string"),
                ("port", "string"),
                ("user", "string"),
            ],
            vec![],
        ),
        (
            "development.ssh.download",
            "SSH Download",
            "Download file via scp",
            vec![
                ("host", "string"),
                ("source", "string"),
                ("destination", "string"),
                ("port", "string"),
                ("user", "string"),
            ],
            vec![],
        ),
        (
            "development.ssh.tunnel",
            "SSH Tunnel",
            "Create SSH tunnel",
            vec![
                ("host", "string"),
                ("local_port", "string"),
                ("remote_port", "string"),
                ("port", "string"),
                ("user", "string"),
            ],
            vec![],
        ),
        // ── Development: Docker ──────────────────────────────
        (
            "development.docker.health",
            "Docker Health",
            "Check docker availability",
            vec![],
            vec![],
        ),
        (
            "development.docker.build",
            "Docker Build",
            "Build an image",
            vec![
                ("path", "string"),
                ("tag", "string"),
                ("dockerfile", "string"),
            ],
            vec![],
        ),
        (
            "development.docker.run",
            "Docker Run",
            "Run a container",
            vec![
                ("image", "string"),
                ("command", "string"),
                ("name", "string"),
                ("detach", "string"),
                ("ports", "string"),
                ("env", "string"),
                ("volumes", "string"),
            ],
            vec![],
        ),
        (
            "development.docker.stop",
            "Docker Stop",
            "Stop a container",
            vec![("container", "string")],
            vec![],
        ),
        (
            "development.docker.start",
            "Docker Start",
            "Start a stopped container",
            vec![("container", "string")],
            vec![],
        ),
        (
            "development.docker.logs",
            "Docker Logs",
            "View container logs",
            vec![
                ("container", "string"),
                ("follow", "string"),
                ("tail", "string"),
            ],
            vec![("output", "string")],
        ),
        (
            "development.docker.exec",
            "Docker Exec",
            "Execute command in container",
            vec![
                ("container", "string"),
                ("command", "string"),
                ("interactive", "string"),
                ("tty", "string"),
            ],
            vec![],
        ),
        (
            "development.docker.ps",
            "Docker PS",
            "List containers",
            vec![("all", "string")],
            vec![("output", "string")],
        ),
        (
            "development.docker.images",
            "Docker Images",
            "List images",
            vec![],
            vec![("output", "string")],
        ),
        (
            "development.docker.pull",
            "Docker Pull",
            "Pull an image",
            vec![("image", "string")],
            vec![],
        ),
        (
            "development.docker.push",
            "Docker Push",
            "Push an image",
            vec![("image", "string")],
            vec![],
        ),
        (
            "development.docker.rm",
            "Docker RM",
            "Remove a container",
            vec![("container", "string"), ("force", "string")],
            vec![],
        ),
        (
            "development.docker.rmi",
            "Docker RMI",
            "Remove an image",
            vec![("image", "string")],
            vec![],
        ),
        (
            "development.docker.network_ls",
            "Docker Networks",
            "List networks",
            vec![],
            vec![("output", "string")],
        ),
        (
            "development.docker.volume_ls",
            "Docker Volumes",
            "List volumes",
            vec![],
            vec![("output", "string")],
        ),
        (
            "development.docker.info",
            "Docker Info",
            "Docker system info",
            vec![],
            vec![("output", "string")],
        ),
        // ── Development: Cargo ───────────────────────────────
        (
            "development.cargo.health",
            "Cargo Health",
            "Check cargo availability",
            vec![],
            vec![],
        ),
        (
            "development.cargo.build",
            "Cargo Build",
            "Build Rust project",
            vec![("release", "string"), ("features", "string")],
            vec![],
        ),
        (
            "development.cargo.test",
            "Cargo Test",
            "Run tests",
            vec![("name", "string")],
            vec![],
        ),
        (
            "development.cargo.run",
            "Cargo Run",
            "Run binary",
            vec![("args", "string")],
            vec![],
        ),
        (
            "development.cargo.check",
            "Cargo Check",
            "Check project",
            vec![],
            vec![],
        ),
        (
            "development.cargo.clippy",
            "Cargo Clippy",
            "Lint project",
            vec![],
            vec![],
        ),
        (
            "development.cargo.fmt",
            "Cargo Fmt",
            "Format code",
            vec![("check", "string")],
            vec![],
        ),
        (
            "development.cargo.doc",
            "Cargo Doc",
            "Build docs",
            vec![("open", "string")],
            vec![],
        ),
        (
            "development.cargo.publish",
            "Cargo Publish",
            "Publish crate",
            vec![],
            vec![],
        ),
        (
            "development.cargo.bench",
            "Cargo Bench",
            "Run benchmarks",
            vec![("name", "string")],
            vec![],
        ),
        (
            "development.cargo.update",
            "Cargo Update",
            "Update dependencies",
            vec![],
            vec![],
        ),
        (
            "development.cargo.clean",
            "Cargo Clean",
            "Clean build artifacts",
            vec![],
            vec![],
        ),
        // ── Development: Rustup ──────────────────────────────
        (
            "development.rustup.health",
            "Rustup Health",
            "Check rustup availability",
            vec![],
            vec![],
        ),
        (
            "development.rustup.install",
            "Rustup Install",
            "Install a toolchain",
            vec![("toolchain", "string")],
            vec![],
        ),
        (
            "development.rustup.update",
            "Rustup Update",
            "Update toolchains",
            vec![],
            vec![],
        ),
        (
            "development.rustup.default",
            "Rustup Default",
            "Set default toolchain",
            vec![("toolchain", "string")],
            vec![],
        ),
        (
            "development.rustup.toolchain_list",
            "Rustup Toolchains",
            "List installed toolchains",
            vec![],
            vec![("output", "string")],
        ),
        (
            "development.rustup.target_add",
            "Rustup Target Add",
            "Add a target",
            vec![("target", "string")],
            vec![],
        ),
        (
            "development.rustup.target_list",
            "Rustup Targets",
            "List targets",
            vec![],
            vec![("output", "string")],
        ),
        (
            "development.rustup.component_add",
            "Rustup Component Add",
            "Add a component",
            vec![("component", "string")],
            vec![],
        ),
        (
            "development.rustup.component_list",
            "Rustup Components",
            "List components",
            vec![],
            vec![("output", "string")],
        ),
        (
            "development.rustup.show",
            "Rustup Show",
            "Show rustup info",
            vec![],
            vec![("output", "string")],
        ),
        // ── Development: Node / npm / pnpm / yarn ────────────
        (
            "development.node.health",
            "Node Health",
            "Check node availability",
            vec![],
            vec![],
        ),
        (
            "development.node.install",
            "Node Install",
            "Install packages",
            vec![("manager", "string"), ("package", "string")],
            vec![],
        ),
        (
            "development.node.build",
            "Node Build",
            "Build project",
            vec![("manager", "string")],
            vec![],
        ),
        (
            "development.node.test",
            "Node Test",
            "Run tests",
            vec![("manager", "string")],
            vec![],
        ),
        (
            "development.node.run",
            "Node Run",
            "Run a script",
            vec![("script", "string"), ("manager", "string")],
            vec![],
        ),
        (
            "development.node.npm.install",
            "NPM Install",
            "npm install",
            vec![],
            vec![],
        ),
        (
            "development.node.npm.build",
            "NPM Build",
            "npm run build",
            vec![],
            vec![],
        ),
        (
            "development.node.npm.test",
            "NPM Test",
            "npm test",
            vec![],
            vec![],
        ),
        (
            "development.node.npm.run",
            "NPM Run",
            "npm run <script>",
            vec![("script", "string")],
            vec![],
        ),
        (
            "development.node.npm.publish",
            "NPM Publish",
            "npm publish",
            vec![],
            vec![],
        ),
        (
            "development.node.npm.init",
            "NPM Init",
            "npm init -y",
            vec![],
            vec![],
        ),
        (
            "development.node.npm.add",
            "NPM Add",
            "npm install <package>",
            vec![("package", "string")],
            vec![],
        ),
        (
            "development.node.npm.remove",
            "NPM Remove",
            "npm uninstall <package>",
            vec![("package", "string")],
            vec![],
        ),
        (
            "development.node.npm.update",
            "NPM Update",
            "npm update",
            vec![],
            vec![],
        ),
        (
            "development.node.npm.outdated",
            "NPM Outdated",
            "npm outdated",
            vec![],
            vec![("output", "string")],
        ),
        (
            "development.node.npm.ls",
            "NPM List",
            "npm ls --depth=0",
            vec![],
            vec![("output", "string")],
        ),
        (
            "development.node.pnpm.install",
            "PNPM Install",
            "pnpm install",
            vec![],
            vec![],
        ),
        (
            "development.node.pnpm.build",
            "PNPM Build",
            "pnpm run build",
            vec![],
            vec![],
        ),
        (
            "development.node.pnpm.test",
            "PNPM Test",
            "pnpm test",
            vec![],
            vec![],
        ),
        (
            "development.node.pnpm.run",
            "PNPM Run",
            "pnpm run <script>",
            vec![("script", "string")],
            vec![],
        ),
        (
            "development.node.pnpm.add",
            "PNPM Add",
            "pnpm add <package>",
            vec![("package", "string")],
            vec![],
        ),
        (
            "development.node.pnpm.remove",
            "PNPM Remove",
            "pnpm remove <package>",
            vec![("package", "string")],
            vec![],
        ),
        (
            "development.node.yarn.install",
            "Yarn Install",
            "yarn install",
            vec![],
            vec![],
        ),
        (
            "development.node.yarn.build",
            "Yarn Build",
            "yarn build",
            vec![],
            vec![],
        ),
        (
            "development.node.yarn.test",
            "Yarn Test",
            "yarn test",
            vec![],
            vec![],
        ),
        (
            "development.node.yarn.run",
            "Yarn Run",
            "yarn <script>",
            vec![("script", "string")],
            vec![],
        ),
        (
            "development.node.yarn.add",
            "Yarn Add",
            "yarn add <package>",
            vec![("package", "string")],
            vec![],
        ),
        (
            "development.node.yarn.remove",
            "Yarn Remove",
            "yarn remove <package>",
            vec![("package", "string")],
            vec![],
        ),
        // ── Development: Python ──────────────────────────────
        (
            "development.python.health",
            "Python Health",
            "Check python availability",
            vec![],
            vec![],
        ),
        (
            "development.python.install",
            "Python Install",
            "Install pip package",
            vec![("package", "string")],
            vec![],
        ),
        (
            "development.python.run",
            "Python Run",
            "Run a Python script",
            vec![("script", "string"), ("args", "string")],
            vec![],
        ),
        (
            "development.python.test",
            "Python Test",
            "Run pytest",
            vec![],
            vec![],
        ),
        (
            "development.python.pip.install",
            "Pip Install",
            "pip install",
            vec![("package", "string")],
            vec![],
        ),
        (
            "development.python.pip.uninstall",
            "Pip Uninstall",
            "pip uninstall",
            vec![("package", "string")],
            vec![],
        ),
        (
            "development.python.pip.freeze",
            "Pip Freeze",
            "pip freeze",
            vec![],
            vec![("output", "string")],
        ),
        (
            "development.python.pip.list",
            "Pip List",
            "pip list",
            vec![],
            vec![("output", "string")],
        ),
        // ── Development: Go ──────────────────────────────────
        (
            "development.go.health",
            "Go Health",
            "Check go availability",
            vec![],
            vec![],
        ),
        (
            "development.go.build",
            "Go Build",
            "Build Go package",
            vec![("output", "string"), ("package", "string")],
            vec![],
        ),
        (
            "development.go.run",
            "Go Run",
            "Run Go file",
            vec![("file", "string"), ("args", "string")],
            vec![],
        ),
        (
            "development.go.test",
            "Go Test",
            "Run Go tests",
            vec![("package", "string"), ("verbose", "string")],
            vec![],
        ),
        (
            "development.go.mod_init",
            "Go Mod Init",
            "Initialize go module",
            vec![("module", "string")],
            vec![],
        ),
        (
            "development.go.mod_tidy",
            "Go Mod Tidy",
            "Tidy go modules",
            vec![],
            vec![],
        ),
        (
            "development.go.mod_download",
            "Go Mod Download",
            "Download modules",
            vec![],
            vec![],
        ),
        (
            "development.go.get",
            "Go Get",
            "Get a Go package",
            vec![("package", "string")],
            vec![],
        ),
        (
            "development.go.install",
            "Go Install",
            "Install a Go package",
            vec![("package", "string")],
            vec![],
        ),
        (
            "development.go.fmt",
            "Go Fmt",
            "Format Go code",
            vec![("path", "string")],
            vec![],
        ),
        (
            "development.go.vet",
            "Go Vet",
            "Vet Go code",
            vec![("package", "string")],
            vec![],
        ),
        // ── Development: Maven ───────────────────────────────
        (
            "development.maven.health",
            "Maven Health",
            "Check mvn availability",
            vec![],
            vec![],
        ),
        (
            "development.maven.build",
            "Maven Build",
            "mvn compile",
            vec![("skip_tests", "string")],
            vec![],
        ),
        (
            "development.maven.test",
            "Maven Test",
            "mvn test",
            vec![],
            vec![],
        ),
        (
            "development.maven.clean",
            "Maven Clean",
            "mvn clean",
            vec![],
            vec![],
        ),
        (
            "development.maven.package",
            "Maven Package",
            "mvn package",
            vec![("skip_tests", "string")],
            vec![],
        ),
        (
            "development.maven.install",
            "Maven Install",
            "mvn install",
            vec![("skip_tests", "string")],
            vec![],
        ),
        (
            "development.maven.deploy",
            "Maven Deploy",
            "mvn deploy",
            vec![],
            vec![],
        ),
        (
            "development.maven.validate",
            "Maven Validate",
            "mvn validate",
            vec![],
            vec![],
        ),
        // ── Development: Gradle ──────────────────────────────
        (
            "development.gradle.health",
            "Gradle Health",
            "Check gradle availability",
            vec![],
            vec![],
        ),
        (
            "development.gradle.build",
            "Gradle Build",
            "gradle build",
            vec![],
            vec![],
        ),
        (
            "development.gradle.test",
            "Gradle Test",
            "gradle test",
            vec![],
            vec![],
        ),
        (
            "development.gradle.clean",
            "Gradle Clean",
            "gradle clean",
            vec![],
            vec![],
        ),
        (
            "development.gradle.run",
            "Gradle Run",
            "gradle run",
            vec![],
            vec![],
        ),
        (
            "development.gradle.assemble",
            "Gradle Assemble",
            "gradle assemble",
            vec![],
            vec![],
        ),
        (
            "development.gradle.check",
            "Gradle Check",
            "gradle check",
            vec![],
            vec![],
        ),
    ]
}

pub fn all_execution_capabilities() -> Vec<ExecutionCapability> {
    all_capabilities()
        .into_iter()
        .map(|(id, name, desc, inputs, outputs)| {
            let mut input_schema = HashMap::new();
            for (k, v) in inputs {
                input_schema.insert(k.to_string(), v.to_string());
            }
            let mut output_schema = HashMap::new();
            for (k, v) in outputs {
                output_schema.insert(k.to_string(), v.to_string());
            }
            ExecutionCapability {
                id: id.to_string(),
                name: name.to_string(),
                description: desc.to_string(),
                input_schema,
                output_schema,
                required_permissions: Vec::new(),
            }
        })
        .collect()
}

pub async fn register_all(registry: &dyn ToolRegistry) -> ExecutionResult<()> {
    let caps = all_execution_capabilities();
    for c in &caps {
        let binding = capability_binding(&c.id);
        registry.register(binding, vec![c.clone()]).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use execution_core::ToolBinding;
    use execution_registry::InMemoryToolRegistry;

    #[test]
    fn test_all_capabilities_count() {
        let caps = all_capabilities();
        assert_eq!(caps.len(), 68);
    }

    #[test]
    fn test_all_execution_capabilities_have_ids() {
        let caps = all_execution_capabilities();
        for c in &caps {
            assert!(!c.id.is_empty(), "capability id must not be empty");
            assert!(
                !c.name.is_empty(),
                "capability name must not be empty: {}",
                c.id
            );
        }
    }

    #[tokio::test]
    async fn test_register_all() {
        let registry = InMemoryToolRegistry::new();
        register_all(&registry).await.unwrap();
        let listed = registry.list_capabilities().await.unwrap();
        assert_eq!(listed.len(), 68);
    }

    #[tokio::test]
    async fn test_register_and_resolve() {
        let registry = InMemoryToolRegistry::new();
        register_all(&registry).await.unwrap();
        let bindings = registry.resolve("desktop.window.list").await.unwrap();
        assert_eq!(bindings.len(), 1);
        let binding = &bindings[0];
        match binding {
            ToolBinding::Subprocess { binary, .. } => {
                assert!(binary.starts_with("@capability/"));
            }
            _ => panic!("expected Subprocess binding"),
        }
    }
}
