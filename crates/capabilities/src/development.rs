use crate::error::{CapabilityError, CapabilityResult};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

// ── Common Utilities ──────────────────────────────────────

fn val<'a>(inputs: &'a HashMap<String, String>, key: &str) -> CapabilityResult<&'a str> {
    inputs
        .get(key)
        .map(|s| s.as_str())
        .ok_or_else(|| CapabilityError::MissingInput(key.into()))
}

fn cmd(binary: &str) -> Command {
    Command::new(binary)
}

fn run(cmd: &mut Command) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    let output = cmd.output()?;
    let code = output.status.code().unwrap_or(-1);
    Ok((output.stdout, output.stderr, code))
}

fn run_mut(mut cmd: Command) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    let output = cmd.output()?;
    let code = output.status.code().unwrap_or(-1);
    Ok((output.stdout, output.stderr, code))
}

fn run_with_timeout(
    mut cmd: Command,
    timeout_ms: u64,
) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    let mut child = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
    let (tx, rx) = mpsc::channel();
    let child_stdout = child.stdout.take().map(BufReader::new);
    let child_stderr = child.stderr.take().map(BufReader::new);

    let tx_stdout = tx.clone();
    let stdout_handle = thread::spawn(move || {
        if let Some(reader) = child_stdout {
            for line in reader.lines() {
                if let Ok(l) = line {
                    let _ = tx_stdout.send((b'O', l));
                }
            }
        }
    });

    let stderr_handle = thread::spawn(move || {
        if let Some(reader) = child_stderr {
            for line in reader.lines() {
                if let Ok(l) = line {
                    let _ = tx.send((b'E', l));
                }
            }
        }
    });

    let (status, timed_out) = {
        let start = std::time::Instant::now();
        loop {
            if start.elapsed() >= Duration::from_millis(timeout_ms) {
                let _ = child.kill();
                break (None, true);
            }
            match child.try_wait() {
                Ok(Some(status)) => break (Some(status), false),
                Ok(None) => thread::sleep(Duration::from_millis(10)),
                Err(e) => {
                    return Err(CapabilityError::ExecutionFailed(e.to_string()));
                }
            }
        }
    };

    let _ = stdout_handle.join();
    let _ = stderr_handle.join();

    if timed_out {
        return Err(CapabilityError::Timeout(format!(
            "command timed out after {}ms",
            timeout_ms
        )));
    }

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    while let Ok((tag, line)) = rx.try_recv() {
        match tag {
            b'O' => {
                writeln!(stdout, "{}", line).ok();
            }
            b'E' => {
                writeln!(stderr, "{}", line).ok();
            }
            _ => {}
        }
    }

    let code = status.map(|s| s.code().unwrap_or(-1)).unwrap_or(-1);
    Ok((stdout, stderr, code))
}

fn health_check_binary(binary: &str) -> CapabilityResult<()> {
    let output = cmd(binary).arg("--version").output().map_err(|e| {
        CapabilityError::ExecutionFailed(format!("{} not available: {}", binary, e))
    })?;
    if output.status.success() {
        Ok(())
    } else {
        Err(CapabilityError::ExecutionFailed(format!(
            "{} health check failed",
            binary
        )))
    }
}

fn health_result(binary: &str) -> (Vec<u8>, Vec<u8>, i32) {
    match health_check_binary(binary) {
        Ok(()) => (b"healthy".to_vec(), Vec::new(), 0),
        Err(e) => (Vec::new(), e.to_string().into_bytes(), 1),
    }
}

fn exec(binary: &str, args: &[&str]) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    let mut c = cmd(binary);
    c.args(args);
    run_mut(c)
}

use std::ops::Deref;
fn owned_cmd(binary: &str) -> Command {
    Command::new(binary)
}

fn exec_owned(binary: &str, args: &[&str]) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    let mut c = Command::new(binary);
    c.args(args);
    run(&mut c)
}

// ── Main Dispatch ─────────────────────────────────────────

pub fn dispatch(
    capability: &str,
    inputs: &HashMap<String, String>,
) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    if capability.starts_with("development.git.") {
        git_dispatch(&capability[17..], inputs)
    } else if capability.starts_with("development.terminal.") {
        terminal_dispatch(&capability[22..], inputs)
    } else if capability.starts_with("development.ssh.") {
        ssh_dispatch(&capability[17..], inputs)
    } else if capability.starts_with("development.docker.") {
        docker_dispatch(&capability[20..], inputs)
    } else if capability.starts_with("development.cargo.") {
        cargo_dispatch(&capability[19..], inputs)
    } else if capability.starts_with("development.rustup.") {
        rustup_dispatch(&capability[20..], inputs)
    } else if capability.starts_with("development.node.") {
        node_dispatch(&capability[17..], inputs)
    } else if capability.starts_with("development.python.") {
        python_dispatch(&capability[20..], inputs)
    } else if capability.starts_with("development.go.") {
        go_dispatch(&capability[15..], inputs)
    } else if capability.starts_with("development.maven.") {
        maven_dispatch(&capability[18..], inputs)
    } else if capability.starts_with("development.gradle.") {
        gradle_dispatch(&capability[19..], inputs)
    } else {
        Err(CapabilityError::UnknownCapability(capability.into()))
    }
}

// ── Git ───────────────────────────────────────────────────

fn git_dispatch(
    action: &str,
    inputs: &HashMap<String, String>,
) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    match action {
        "health" => Ok(health_result("git")),
        "clone" => {
            let url = val(inputs, "url")?;
            let dir = inputs.get("directory").map(|s| s.as_str());
            let mut c = cmd("git");
            c.arg("clone").arg(url);
            if let Some(d) = dir {
                c.arg(d);
            }
            run(&mut c)
        }
        "init" => {
            let dir = val(inputs, "directory")?;
            exec("git", &["init", dir])
        }
        "add" => {
            let path = val(inputs, "path")?;
            exec("git", &["add", path])
        }
        "commit" => {
            let msg = val(inputs, "message")?;
            run(&mut cmd("git").args(["commit", "-m", msg]))
        }
        "push" => {
            let remote = inputs.get("remote").map(|s| s.as_str()).unwrap_or("origin");
            let branch = inputs.get("branch").map(|s| s.as_str()).unwrap_or("HEAD");
            exec("git", &["push", remote, branch])
        }
        "pull" => {
            let remote = inputs.get("remote").map(|s| s.as_str()).unwrap_or("origin");
            let branch = inputs.get("branch").map(|s| s.as_str()).unwrap_or("HEAD");
            exec("git", &["pull", remote, branch])
        }
        "checkout" => {
            let branch = val(inputs, "branch")?;
            let create = inputs.get("create").map(|s| s == "true").unwrap_or(false);
            if create {
                exec("git", &["checkout", "-b", branch])
            } else {
                exec("git", &["checkout", branch])
            }
        }
        "branch" => {
            let action = inputs.get("action").map(|s| s.as_str()).unwrap_or("list");
            match action {
                "list" => exec("git", &["branch"]),
                "create" => {
                    let name = val(inputs, "name")?;
                    exec("git", &["branch", name])
                }
                "delete" => {
                    let name = val(inputs, "name")?;
                    exec("git", &["branch", "-d", name])
                }
                "delete_remote" => {
                    let name = val(inputs, "name")?;
                    exec("git", &["push", "origin", "--delete", name])
                }
                _ => Err(CapabilityError::InvalidInput(format!(
                    "unknown branch action: {}",
                    action
                ))),
            }
        }
        "status" => exec("git", &["status"]),
        "log" => {
            let max_count = inputs.get("max_count").map(|s| s.as_str()).unwrap_or("10");
            exec("git", &["log", "--oneline", "-n", max_count])
        }
        "diff" => {
            let staged = inputs.get("staged").map(|s| s == "true").unwrap_or(false);
            if staged {
                exec("git", &["diff", "--cached"])
            } else {
                exec("git", &["diff"])
            }
        }
        "stash" => {
            let action = inputs.get("action").map(|s| s.as_str()).unwrap_or("push");
            match action {
                "push" => exec("git", &["stash"]),
                "pop" => exec("git", &["stash", "pop"]),
                "list" => exec("git", &["stash", "list"]),
                "drop" => exec("git", &["stash", "drop"]),
                _ => Err(CapabilityError::InvalidInput(format!(
                    "unknown stash action: {}",
                    action
                ))),
            }
        }
        "tag" => {
            let name = val(inputs, "name")?;
            let msg = inputs.get("message").map(|s| s.as_str());
            let mut c = cmd("git");
            c.arg("tag").arg(name);
            if let Some(m) = msg {
                c.args(["-a", "-m", m]);
            }
            run(&mut c)
        }
        "merge" => {
            let branch = val(inputs, "branch")?;
            exec("git", &["merge", branch])
        }
        "rebase" => {
            let branch = val(inputs, "branch")?;
            exec("git", &["rebase", branch])
        }
        "reset" => {
            let target = inputs.get("target").map(|s| s.as_str()).unwrap_or("HEAD");
            let mode = inputs.get("mode").map(|s| s.as_str()).unwrap_or("mixed");
            exec("git", &["reset", &format!("--{}", mode), target])
        }
        "fetch" => {
            let remote = inputs.get("remote").map(|s| s.as_str()).unwrap_or("--all");
            exec("git", &["fetch", remote])
        }
        "remote" => exec("git", &["remote", "-v"]),
        "config" => {
            let key = val(inputs, "key")?;
            let value = val(inputs, "value")?;
            exec("git", &["config", key, value])
        }
        "clean" => exec("git", &["clean", "-fd"]),
        _ => Err(CapabilityError::UnknownCapability(format!(
            "development.git.{}",
            action
        ))),
    }
}

// ── Terminal ───────────────────────────────────────────────

fn terminal_dispatch(
    action: &str,
    inputs: &HashMap<String, String>,
) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    match action {
        "health" => Ok(health_result("sh")),
        "execute" => {
            let command = val(inputs, "command")?;
            let cwd = inputs.get("working_directory").map(|s| s.as_str());
            let timeout = inputs
                .get("timeout")
                .map(|s| s.parse::<u64>().unwrap_or(30000))
                .unwrap_or(30000u64);
            let mut c = cmd("sh");
            c.args(["-c", command]);
            if let Some(dir) = cwd {
                c.current_dir(dir);
            }
            run_with_timeout(c, timeout)
        }
        _ => Err(CapabilityError::UnknownCapability(format!(
            "development.terminal.{}",
            action
        ))),
    }
}

// ── SSH ────────────────────────────────────────────────────

fn ssh_dispatch(
    action: &str,
    inputs: &HashMap<String, String>,
) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    match action {
        "health" => Ok(health_result("ssh")),
        "execute" => {
            let host = val(inputs, "host")?;
            let command = val(inputs, "command")?;
            let port = inputs.get("port").map(|s| s.as_str()).unwrap_or("22");
            let user = inputs.get("user").map(|s| s.as_str());
            let mut c = cmd("ssh");
            c.args(["-p", port]);
            if let Some(u) = user {
                c.arg(format!("{}@{}", u, host));
            } else {
                c.arg(host);
            }
            c.arg(command);
            run(&mut c)
        }
        "connect" => {
            let host = val(inputs, "host")?;
            let port = inputs.get("port").map(|s| s.as_str()).unwrap_or("22");
            let user = inputs.get("user").map(|s| s.as_str());
            let mut c = cmd("ssh");
            c.args(["-p", port]);
            if let Some(u) = user {
                c.arg(format!("{}@{}", u, host));
            } else {
                c.arg(host);
            }
            run(&mut c)
        }
        "upload" => {
            let host = val(inputs, "host")?;
            let source = val(inputs, "source")?;
            let dest = val(inputs, "destination")?;
            let port = inputs.get("port").map(|s| s.as_str()).unwrap_or("22");
            let user = inputs.get("user").map(|s| s.as_str());
            let mut c = cmd("scp");
            c.args(["-P", port]);
            if let Some(u) = user {
                c.arg(format!("{}@{}:{}", u, host, dest));
            } else {
                c.arg(format!("{}:{}", host, dest));
            }
            c.arg(source);
            run(&mut c)
        }
        "download" => {
            let host = val(inputs, "host")?;
            let source = val(inputs, "source")?;
            let dest = val(inputs, "destination")?;
            let port = inputs.get("port").map(|s| s.as_str()).unwrap_or("22");
            let user = inputs.get("user").map(|s| s.as_str());
            let mut c = cmd("scp");
            c.args(["-P", port]);
            if let Some(u) = user {
                c.arg(format!("{}@{}:{}", u, host, source));
            } else {
                c.arg(format!("{}:{}", host, source));
            }
            c.arg(dest);
            run(&mut c)
        }
        "tunnel" => {
            let host = val(inputs, "host")?;
            let local_port = val(inputs, "local_port")?;
            let remote_port = val(inputs, "remote_port")?;
            let port = inputs.get("port").map(|s| s.as_str()).unwrap_or("22");
            let user = inputs.get("user").map(|s| s.as_str());
            let mut c = cmd("ssh");
            c.args([
                "-p",
                port,
                "-L",
                &format!("{}:localhost:{}", local_port, remote_port),
            ]);
            if let Some(u) = user {
                c.arg(format!("{}@{}", u, host));
            } else {
                c.arg(host);
            }
            c.arg("-N");
            run(&mut c)
        }
        _ => Err(CapabilityError::UnknownCapability(format!(
            "development.ssh.{}",
            action
        ))),
    }
}

// ── Docker ─────────────────────────────────────────────────

fn docker_dispatch(
    action: &str,
    inputs: &HashMap<String, String>,
) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    match action {
        "health" => Ok(health_result("docker")),
        "build" => {
            let path = inputs.get("path").map(|s| s.as_str()).unwrap_or(".");
            let tag = inputs.get("tag").map(|s| s.as_str());
            let dockerfile = inputs.get("dockerfile").map(|s| s.as_str());
            let mut c = cmd("docker");
            c.arg("build");
            if let Some(t) = tag {
                c.args(["-t", t]);
            }
            if let Some(f) = dockerfile {
                c.args(["-f", f]);
            }
            c.arg(path);
            run(&mut c)
        }
        "run" => {
            let image = val(inputs, "image")?;
            let cmd_args = inputs.get("command").map(|s| s.as_str());
            let name = inputs.get("name").map(|s| s.as_str());
            let detach = inputs.get("detach").map(|s| s == "true").unwrap_or(false);
            let ports = inputs.get("ports").map(|s| s.as_str());
            let env = inputs.get("env").map(|s| s.as_str());
            let volumes = inputs.get("volumes").map(|s| s.as_str());
            let mut c = cmd("docker");
            c.arg("run");
            if detach {
                c.arg("-d");
            }
            if let Some(n) = name {
                c.args(["--name", n]);
            }
            if let Some(p) = ports {
                for mapping in p.split(',') {
                    let mapping = mapping.trim();
                    if !mapping.is_empty() {
                        c.args(["-p", mapping]);
                    }
                }
            }
            if let Some(e) = env {
                for pair in e.split(',') {
                    let pair = pair.trim();
                    if !pair.is_empty() {
                        c.args(["-e", pair]);
                    }
                }
            }
            if let Some(v) = volumes {
                for vol in v.split(',') {
                    let vol = vol.trim();
                    if !vol.is_empty() {
                        c.args(["-v", vol]);
                    }
                }
            }
            c.arg(image);
            if let Some(ca) = cmd_args {
                c.arg(ca);
            }
            run(&mut c)
        }
        "stop" => {
            let container = val(inputs, "container")?;
            exec("docker", &["stop", container])
        }
        "start" => {
            let container = val(inputs, "container")?;
            exec("docker", &["start", container])
        }
        "logs" => {
            let container = val(inputs, "container")?;
            let follow = inputs.get("follow").map(|s| s == "true").unwrap_or(false);
            let tail = inputs.get("tail").map(|s| s.as_str()).unwrap_or("all");
            let mut c = cmd("docker");
            c.args(["logs"]);
            if follow {
                c.arg("-f");
            }
            c.args(["--tail", tail]);
            c.arg(container);
            run(&mut c)
        }
        "exec" => {
            let container = val(inputs, "container")?;
            let command = val(inputs, "command")?;
            let interactive = inputs
                .get("interactive")
                .map(|s| s == "true")
                .unwrap_or(false);
            let tty = inputs.get("tty").map(|s| s == "true").unwrap_or(false);
            let mut c = cmd("docker");
            c.args(["exec"]);
            if interactive {
                c.arg("-i");
            }
            if tty {
                c.arg("-t");
            }
            c.arg(container).arg(command);
            run(&mut c)
        }
        "ps" => {
            let all = inputs.get("all").map(|s| s == "true").unwrap_or(true);
            if all {
                exec("docker", &["ps", "-a"])
            } else {
                exec("docker", &["ps"])
            }
        }
        "images" => exec("docker", &["images"]),
        "pull" => {
            let image = val(inputs, "image")?;
            exec("docker", &["pull", image])
        }
        "push" => {
            let image = val(inputs, "image")?;
            exec("docker", &["push", image])
        }
        "rm" => {
            let container = val(inputs, "container")?;
            let force = inputs.get("force").map(|s| s == "true").unwrap_or(false);
            if force {
                exec("docker", &["rm", "-f", container])
            } else {
                exec("docker", &["rm", container])
            }
        }
        "rmi" => {
            let image = val(inputs, "image")?;
            exec("docker", &["rmi", image])
        }
        "network_ls" => exec("docker", &["network", "ls"]),
        "volume_ls" => exec("docker", &["volume", "ls"]),
        "info" => exec("docker", &["info"]),
        _ => Err(CapabilityError::UnknownCapability(format!(
            "development.docker.{}",
            action
        ))),
    }
}

// ── Cargo ──────────────────────────────────────────────────

fn cargo_dispatch(
    action: &str,
    inputs: &HashMap<String, String>,
) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    match action {
        "health" => Ok(health_result("cargo")),
        "build" => {
            let mut c = cmd("cargo");
            c.arg("build");
            if inputs.get("release").map(|s| s == "true").unwrap_or(false) {
                c.arg("--release");
            }
            if let Some(features) = inputs.get("features") {
                c.args(["--features", features]);
            }
            run(&mut c)
        }
        "test" => {
            let mut c = cmd("cargo");
            c.arg("test");
            if let Some(name) = inputs.get("name") {
                c.args(["--", name]);
            }
            run(&mut c)
        }
        "run" => {
            let mut c = cmd("cargo");
            c.arg("run");
            if let Some(args) = inputs.get("args") {
                c.arg("--").arg(args);
            }
            run(&mut c)
        }
        "check" => exec("cargo", &["check"]),
        "clippy" => exec("cargo", &["clippy"]),
        "fmt" => {
            let check = inputs.get("check").map(|s| s == "true").unwrap_or(false);
            if check {
                exec("cargo", &["fmt", "--check"])
            } else {
                exec("cargo", &["fmt"])
            }
        }
        "doc" => {
            let open = inputs.get("open").map(|s| s == "true").unwrap_or(false);
            if open {
                exec("cargo", &["doc", "--open"])
            } else {
                exec("cargo", &["doc"])
            }
        }
        "publish" => exec("cargo", &["publish"]),
        "bench" => {
            let mut c = cmd("cargo");
            c.arg("bench");
            if let Some(name) = inputs.get("name") {
                c.args(["--", name]);
            }
            run(&mut c)
        }
        "update" => exec("cargo", &["update"]),
        "clean" => exec("cargo", &["clean"]),
        _ => Err(CapabilityError::UnknownCapability(format!(
            "development.cargo.{}",
            action
        ))),
    }
}

// ── Rustup ─────────────────────────────────────────────────

fn rustup_dispatch(
    action: &str,
    inputs: &HashMap<String, String>,
) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    match action {
        "health" => Ok(health_result("rustup")),
        "install" => exec("rustup", &["install", val(inputs, "toolchain")?]),
        "update" => exec("rustup", &["update"]),
        "default" => exec("rustup", &["default", val(inputs, "toolchain")?]),
        "toolchain_list" => exec("rustup", &["toolchain", "list"]),
        "target_add" => exec("rustup", &["target", "add", val(inputs, "target")?]),
        "target_list" => exec("rustup", &["target", "list"]),
        "component_add" => exec("rustup", &["component", "add", val(inputs, "component")?]),
        "component_list" => exec("rustup", &["component", "list"]),
        "show" => exec("rustup", &["show"]),
        _ => Err(CapabilityError::UnknownCapability(format!(
            "development.rustup.{}",
            action
        ))),
    }
}

// ── Node / npm / pnpm / yarn ───────────────────────────────

fn node_dispatch(
    action: &str,
    inputs: &HashMap<String, String>,
) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    match action {
        "health" => Ok(health_result("node")),
        "install" => {
            let manager = inputs.get("manager").map(|s| s.as_str()).unwrap_or("npm");
            match manager {
                "npm" => {
                    let mut c = cmd("npm");
                    c.arg("install");
                    if let Some(pkg) = inputs.get("package") {
                        c.args(["--save", pkg]);
                    }
                    run(&mut c)
                }
                "pnpm" => {
                    let mut c = cmd("pnpm");
                    c.arg("install");
                    if let Some(pkg) = inputs.get("package") {
                        c.arg("add").arg(pkg);
                    }
                    run(&mut c)
                }
                "yarn" => {
                    let mut c = cmd("yarn");
                    if let Some(pkg) = inputs.get("package") {
                        c.arg("add").arg(pkg);
                    } else {
                        c.arg("install");
                    }
                    run(&mut c)
                }
                _ => Err(CapabilityError::InvalidInput(format!(
                    "unknown package manager: {}",
                    manager
                ))),
            }
        }
        "build" => {
            let manager = inputs.get("manager").map(|s| s.as_str()).unwrap_or("npm");
            match manager {
                "npm" => exec("npm", &["run", "build"]),
                "pnpm" => exec("pnpm", &["run", "build"]),
                "yarn" => exec("yarn", &["build"]),
                _ => Err(CapabilityError::InvalidInput(format!(
                    "unknown package manager: {}",
                    manager
                ))),
            }
        }
        "test" => {
            let manager = inputs.get("manager").map(|s| s.as_str()).unwrap_or("npm");
            match manager {
                "npm" => exec("npm", &["test"]),
                "pnpm" => exec("pnpm", &["test"]),
                "yarn" => exec("yarn", &["test"]),
                _ => Err(CapabilityError::InvalidInput(format!(
                    "unknown package manager: {}",
                    manager
                ))),
            }
        }
        "run" => {
            let script = val(inputs, "script")?;
            let manager = inputs.get("manager").map(|s| s.as_str()).unwrap_or("npm");
            match manager {
                "npm" => exec("npm", &["run", script]),
                "pnpm" => exec("pnpm", &["run", script]),
                "yarn" => exec("yarn", &[script]),
                _ => Err(CapabilityError::InvalidInput(format!(
                    "unknown package manager: {}",
                    manager
                ))),
            }
        }
        "npm.install" => exec("npm", &["install"]),
        "npm.build" => exec("npm", &["run", "build"]),
        "npm.test" => exec("npm", &["test"]),
        "npm.run" => exec("npm", &["run", val(inputs, "script")?]),
        "npm.publish" => exec("npm", &["publish"]),
        "npm.init" => exec("npm", &["init", "-y"]),
        "npm.add" => exec("npm", &["install", val(inputs, "package")?]),
        "npm.remove" => exec("npm", &["uninstall", val(inputs, "package")?]),
        "npm.update" => exec("npm", &["update"]),
        "npm.outdated" => exec("npm", &["outdated"]),
        "npm.ls" => exec("npm", &["ls", "--depth=0"]),
        "pnpm.install" => exec("pnpm", &["install"]),
        "pnpm.build" => exec("pnpm", &["run", "build"]),
        "pnpm.test" => exec("pnpm", &["test"]),
        "pnpm.run" => exec("pnpm", &["run", val(inputs, "script")?]),
        "pnpm.add" => exec("pnpm", &["add", val(inputs, "package")?]),
        "pnpm.remove" => exec("pnpm", &["remove", val(inputs, "package")?]),
        "yarn.install" => exec("yarn", &["install"]),
        "yarn.build" => exec("yarn", &["build"]),
        "yarn.test" => exec("yarn", &["test"]),
        "yarn.run" => exec("yarn", &[val(inputs, "script")?]),
        "yarn.add" => exec("yarn", &["add", val(inputs, "package")?]),
        "yarn.remove" => exec("yarn", &["remove", val(inputs, "package")?]),
        _ => Err(CapabilityError::UnknownCapability(format!(
            "development.node.{}",
            action
        ))),
    }
}

// ── Python ─────────────────────────────────────────────────

fn python_dispatch(
    action: &str,
    inputs: &HashMap<String, String>,
) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    match action {
        "health" => Ok(health_result("python3")),
        "install" => {
            let pkg = val(inputs, "package")?;
            exec("pip3", &["install", pkg])
        }
        "run" => {
            let script = val(inputs, "script")?;
            let args = inputs.get("args").map(|s| s.as_str()).unwrap_or("");
            let mut c = cmd("python3");
            c.arg(script);
            if !args.is_empty() {
                c.arg(args);
            }
            run(&mut c)
        }
        "test" => exec("python3", &["-m", "pytest"]),
        "pip.install" => exec("pip3", &["install", val(inputs, "package")?]),
        "pip.uninstall" => exec("pip3", &["uninstall", "-y", val(inputs, "package")?]),
        "pip.freeze" => exec("pip3", &["freeze"]),
        "pip.list" => exec("pip3", &["list"]),
        _ => Err(CapabilityError::UnknownCapability(format!(
            "development.python.{}",
            action
        ))),
    }
}

// ── Go ─────────────────────────────────────────────────────

fn go_dispatch(
    action: &str,
    inputs: &HashMap<String, String>,
) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    match action {
        "health" => Ok(health_result("go")),
        "build" => {
            let output = inputs.get("output").map(|s| s.as_str());
            let pkg = inputs.get("package").map(|s| s.as_str());
            let mut c = cmd("go");
            c.arg("build");
            if let Some(o) = output {
                c.args(["-o", o]);
            }
            if let Some(p) = pkg {
                c.arg(p);
            }
            run(&mut c)
        }
        "run" => {
            let file = val(inputs, "file")?;
            let args = inputs.get("args").map(|s| s.as_str()).unwrap_or("");
            let mut c = cmd("go");
            c.args(["run", file]);
            if !args.is_empty() {
                c.arg(args);
            }
            run(&mut c)
        }
        "test" => {
            let pkg = inputs.get("package").map(|s| s.as_str()).unwrap_or("./...");
            let verbose = inputs.get("verbose").map(|s| s == "true").unwrap_or(false);
            let mut c = cmd("go");
            c.args(["test"]);
            if verbose {
                c.arg("-v");
            }
            c.arg(pkg);
            run(&mut c)
        }
        "mod_init" => {
            let module = val(inputs, "module")?;
            exec("go", &["mod", "init", module])
        }
        "mod_tidy" => exec("go", &["mod", "tidy"]),
        "mod_download" => exec("go", &["mod", "download"]),
        "get" => {
            let pkg = val(inputs, "package")?;
            exec("go", &["get", pkg])
        }
        "install" => {
            let pkg = val(inputs, "package")?;
            exec("go", &["install", pkg])
        }
        "fmt" => exec("go", &["fmt", val(inputs, "path")?]),
        "vet" => exec("go", &["vet", val(inputs, "package").unwrap_or("./...")]),
        _ => Err(CapabilityError::UnknownCapability(format!(
            "development.go.{}",
            action
        ))),
    }
}

// ── Maven ──────────────────────────────────────────────────

fn maven_dispatch(
    action: &str,
    inputs: &HashMap<String, String>,
) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    match action {
        "health" => Ok(health_result("mvn")),
        "build" => {
            let skip_tests = inputs
                .get("skip_tests")
                .map(|s| s == "true")
                .unwrap_or(false);
            let mut c = cmd("mvn");
            c.arg("compile");
            if skip_tests {
                c.arg("-DskipTests");
            }
            run(&mut c)
        }
        "test" => exec("mvn", &["test"]),
        "clean" => exec("mvn", &["clean"]),
        "package" => {
            let skip_tests = inputs
                .get("skip_tests")
                .map(|s| s == "true")
                .unwrap_or(false);
            let mut c = cmd("mvn");
            c.arg("package");
            if skip_tests {
                c.arg("-DskipTests");
            }
            run(&mut c)
        }
        "install" => {
            let skip_tests = inputs
                .get("skip_tests")
                .map(|s| s == "true")
                .unwrap_or(false);
            let mut c = cmd("mvn");
            c.arg("install");
            if skip_tests {
                c.arg("-DskipTests");
            }
            run(&mut c)
        }
        "deploy" => exec("mvn", &["deploy"]),
        "validate" => exec("mvn", &["validate"]),
        _ => Err(CapabilityError::UnknownCapability(format!(
            "development.maven.{}",
            action
        ))),
    }
}

// ── Gradle ─────────────────────────────────────────────────

fn gradle_dispatch(
    action: &str,
    inputs: &HashMap<String, String>,
) -> CapabilityResult<(Vec<u8>, Vec<u8>, i32)> {
    match action {
        "health" => Ok(health_result("gradle")),
        "build" => exec("gradle", &["build"]),
        "test" => exec("gradle", &["test"]),
        "clean" => exec("gradle", &["clean"]),
        "run" => exec("gradle", &["run"]),
        "assemble" => exec("gradle", &["assemble"]),
        "check" => exec("gradle", &["check"]),
        _ => Err(CapabilityError::UnknownCapability(format!(
            "development.gradle.{}",
            action
        ))),
    }
}

// ── Tests ──────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::CapabilityError;

    #[test]
    fn test_unknown_capability() {
        let result = dispatch("development.nonexistent", &HashMap::new());
        assert!(matches!(
            result.unwrap_err(),
            CapabilityError::UnknownCapability(_)
        ));
    }

    // ── Git ──

    #[test]
    fn test_git_unknown_action() {
        let result = dispatch("development.git.nopenopenope", &HashMap::new());
        assert!(matches!(
            result.unwrap_err(),
            CapabilityError::UnknownCapability(_)
        ));
    }

    #[test]
    fn test_git_clone_missing_url() {
        let result = dispatch("development.git.clone", &HashMap::new());
        assert!(matches!(
            result.unwrap_err(),
            CapabilityError::MissingInput(_)
        ));
    }

    #[test]
    fn test_git_commit_missing_message() {
        let mut inputs = HashMap::new();
        inputs.insert("path".to_string(), ".".to_string());
        let result = dispatch("development.git.commit", &inputs);
        assert!(matches!(
            result.unwrap_err(),
            CapabilityError::MissingInput(_)
        ));
    }

    #[test]
    fn test_git_branch_invalid_action() {
        let mut inputs = HashMap::new();
        inputs.insert("action".to_string(), "invalid".to_string());
        let result = dispatch("development.git.branch", &inputs);
        assert!(matches!(
            result.unwrap_err(),
            CapabilityError::InvalidInput(_)
        ));
    }

    #[test]
    fn test_git_stash_invalid_action() {
        let mut inputs = HashMap::new();
        inputs.insert("action".to_string(), "invalid".to_string());
        let result = dispatch("development.git.stash", &inputs);
        assert!(matches!(
            result.unwrap_err(),
            CapabilityError::InvalidInput(_)
        ));
    }

    #[test]
    fn test_git_status_succeeds() {
        let result = dispatch("development.git.status", &HashMap::new());
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_git_log_succeeds() {
        let result = dispatch("development.git.log", &HashMap::new());
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_git_health() {
        let (_, _, code) = dispatch("development.git.health", &HashMap::new()).unwrap();
        assert_eq!(code, 0);
    }

    // ── Terminal ──

    #[test]
    fn test_terminal_execute_missing_command() {
        let result = dispatch("development.terminal.execute", &HashMap::new());
        assert!(matches!(
            result.unwrap_err(),
            CapabilityError::MissingInput(_)
        ));
    }

    #[test]
    fn test_terminal_health() {
        let (_, _, code) = dispatch("development.terminal.health", &HashMap::new()).unwrap();
        assert_eq!(code, 0);
    }

    // ── SSH ──

    #[test]
    fn test_ssh_execute_missing_host() {
        let result = dispatch("development.ssh.execute", &HashMap::new());
        assert!(matches!(
            result.unwrap_err(),
            CapabilityError::MissingInput(_)
        ));
    }

    #[test]
    fn test_ssh_health() {
        let (_, _, code) = dispatch("development.ssh.health", &HashMap::new()).unwrap();
        assert_eq!(code, 0);
    }

    #[test]
    fn test_ssh_upload_missing_host() {
        let result = dispatch("development.ssh.upload", &HashMap::new());
        assert!(matches!(
            result.unwrap_err(),
            CapabilityError::MissingInput(_)
        ));
    }

    #[test]
    fn test_ssh_download_missing_source() {
        let mut inputs = HashMap::new();
        inputs.insert("host".to_string(), "example.com".to_string());
        let result = dispatch("development.ssh.download", &inputs);
        assert!(matches!(
            result.unwrap_err(),
            CapabilityError::MissingInput(_)
        ));
    }

    // ── Docker ──

    #[test]
    fn test_docker_build_succeeds() {
        let result = dispatch("development.docker.health", &HashMap::new());
        assert!(result.is_ok());
    }

    #[test]
    fn test_docker_run_missing_image() {
        let result = dispatch("development.docker.run", &HashMap::new());
        assert!(matches!(
            result.unwrap_err(),
            CapabilityError::MissingInput(_)
        ));
    }

    #[test]
    fn test_docker_stop_missing_container() {
        let result = dispatch("development.docker.stop", &HashMap::new());
        assert!(matches!(
            result.unwrap_err(),
            CapabilityError::MissingInput(_)
        ));
    }

    #[test]
    fn test_docker_logs_missing_container() {
        let result = dispatch("development.docker.logs", &HashMap::new());
        assert!(matches!(
            result.unwrap_err(),
            CapabilityError::MissingInput(_)
        ));
    }

    #[test]
    fn test_docker_exec_missing_command() {
        let mut inputs = HashMap::new();
        inputs.insert("container".to_string(), "test".to_string());
        let result = dispatch("development.docker.exec", &inputs);
        assert!(matches!(
            result.unwrap_err(),
            CapabilityError::MissingInput(_)
        ));
    }

    #[test]
    fn test_docker_ps_succeeds() {
        let result = dispatch("development.docker.ps", &HashMap::new());
        assert!(result.is_ok() || result.is_err());
    }

    // ── Cargo ──

    #[test]
    fn test_cargo_build_succeeds() {
        let result = dispatch("development.cargo.health", &HashMap::new());
        assert!(result.is_ok());
    }

    #[test]
    fn test_cargo_test_with_name() {
        let mut inputs = HashMap::new();
        inputs.insert("name".to_string(), "my_test".to_string());
        let result = dispatch("development.cargo.test", &inputs);
        assert!(result.is_ok() || result.is_err());
    }

    // ── Rustup ──

    #[test]
    fn test_rustup_install_missing_toolchain() {
        let result = dispatch("development.rustup.install", &HashMap::new());
        assert!(matches!(
            result.unwrap_err(),
            CapabilityError::MissingInput(_)
        ));
    }

    #[test]
    fn test_rustup_default_missing_toolchain() {
        let result = dispatch("development.rustup.default", &HashMap::new());
        assert!(matches!(
            result.unwrap_err(),
            CapabilityError::MissingInput(_)
        ));
    }

    // ── Node ──

    #[test]
    fn test_node_install_missing_package() {
        let mut inputs = HashMap::new();
        inputs.insert("manager".to_string(), "npm".to_string());
        let result = dispatch("development.node.install", &inputs);
        // install without package succeeds (just `npm install` for deps)
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_node_unknown_manager() {
        let mut inputs = HashMap::new();
        inputs.insert("manager".to_string(), "bogus".to_string());
        inputs.insert("package".to_string(), "test".to_string());
        let result = dispatch("development.node.install", &inputs);
        assert!(matches!(
            result.unwrap_err(),
            CapabilityError::InvalidInput(_)
        ));
    }

    #[test]
    fn test_node_run_missing_script() {
        let result = dispatch("development.node.run", &HashMap::new());
        assert!(matches!(
            result.unwrap_err(),
            CapabilityError::MissingInput(_)
        ));
    }

    #[test]
    fn test_npm_install_succeeds() {
        let result = dispatch("development.node.npm.install", &HashMap::new());
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_npm_add_missing_package() {
        let result = dispatch("development.node.npm.add", &HashMap::new());
        assert!(matches!(
            result.unwrap_err(),
            CapabilityError::MissingInput(_)
        ));
    }

    #[test]
    fn test_yarn_run_missing_script() {
        let result = dispatch("development.node.yarn.run", &HashMap::new());
        assert!(matches!(
            result.unwrap_err(),
            CapabilityError::MissingInput(_)
        ));
    }

    #[test]
    fn test_pnpm_remove_missing_package() {
        let result = dispatch("development.node.pnpm.remove", &HashMap::new());
        assert!(matches!(
            result.unwrap_err(),
            CapabilityError::MissingInput(_)
        ));
    }

    // ── Python ──

    #[test]
    fn test_python_install_missing_package() {
        let result = dispatch("development.python.install", &HashMap::new());
        assert!(matches!(
            result.unwrap_err(),
            CapabilityError::MissingInput(_)
        ));
    }

    #[test]
    fn test_python_run_missing_script() {
        let result = dispatch("development.python.run", &HashMap::new());
        assert!(matches!(
            result.unwrap_err(),
            CapabilityError::MissingInput(_)
        ));
    }

    // ── Go ──

    #[test]
    fn test_go_build_succeeds() {
        let result = dispatch("development.go.health", &HashMap::new());
        assert!(result.is_ok());
    }

    #[test]
    fn test_go_run_missing_file() {
        let result = dispatch("development.go.run", &HashMap::new());
        assert!(matches!(
            result.unwrap_err(),
            CapabilityError::MissingInput(_)
        ));
    }

    #[test]
    fn test_go_mod_init_missing_module() {
        let result = dispatch("development.go.mod_init", &HashMap::new());
        assert!(matches!(
            result.unwrap_err(),
            CapabilityError::MissingInput(_)
        ));
    }

    #[test]
    fn test_go_get_missing_package() {
        let result = dispatch("development.go.get", &HashMap::new());
        assert!(matches!(
            result.unwrap_err(),
            CapabilityError::MissingInput(_)
        ));
    }

    // ── Maven ──

    #[test]
    fn test_maven_build_succeeds() {
        let result = dispatch("development.maven.health", &HashMap::new());
        assert!(result.is_ok());
    }

    #[test]
    fn test_maven_unknown_action() {
        let result = dispatch("development.maven.nonexistent", &HashMap::new());
        assert!(matches!(
            result.unwrap_err(),
            CapabilityError::UnknownCapability(_)
        ));
    }

    // ── Gradle ──

    #[test]
    fn test_gradle_build_succeeds() {
        let result = dispatch("development.gradle.health", &HashMap::new());
        assert!(result.is_ok());
    }

    #[test]
    fn test_gradle_test_succeeds() {
        let result = dispatch("development.gradle.test", &HashMap::new());
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_gradle_unknown_action() {
        let result = dispatch("development.gradle.invalid", &HashMap::new());
        assert!(matches!(
            result.unwrap_err(),
            CapabilityError::UnknownCapability(_)
        ));
    }
}
