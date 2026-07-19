# Development Guide

## Setting Up the Development Environment

### System Requirements

- Arch Linux (see `scripts/setup.sh` for automated setup)
- Rust toolchain (stable, latest)
- 8 GB RAM minimum (16 GB recommended)
- 20 GB free disk space

### Automated Setup

Run the project's setup script for a fully automated environment:

```bash
./scripts/setup.sh
```

This script installs system packages, configures Git and SSH, and verifies the toolchain.

### Manual Setup

If you prefer a manual setup or need to customize, install the following packages:

```bash
# Essential development tools
sudo pacman -S --needed base-devel git openssh

# Rust toolchain
sudo pacman -S rustup
rustup default stable
rustup component add clippy rustfmt

# Language runtimes
sudo pacman -S gcc clang cmake make go python

# Developer utilities
sudo pacman -S curl wget unzip zip tmux tree htop jq ripgrep fd vim nano

# Build dependencies
sudo pacman -S openssl pkg-config protobuf
```

### Verifying the Setup

```bash
rustc --version
cargo --version
rustup show
cargo fmt -- --version
cargo clippy --version
```

---

## Recommended Tools

### Rust Language Server

```bash
# VS Code / VSCodium / code-server
# Install the rust-analyzer extension from the marketplace

# Or standalone
sudo pacman -S rust-analyzer
```

### Cargo Plugins

```bash
# Watch files and recompile on changes
cargo install cargo-watch

# Run tests with better output
cargo install cargo-nextest

# Run benchmarks
# (Criterion is already a dev-dependency; no install needed)

# Audit dependencies for vulnerabilities
cargo install cargo-audit

# Generate code coverage reports
cargo install cargo-tarpaulin

# Display dependency tree
cargo install cargo-tree  # or use `cargo tree` (built-in)
```

### Development Servers

```bash
# Hot-reload when using the platform binary
cargo install cargo-watch

# Run with automatic recompilation
cargo watch -x run
```

### Profiling Tools

```bash
# System profiling
sudo pacman -S perf

# Flamegraph generation
cargo install flamegraph

# Heap profiling
cargo install dhat-viewer
# Or use jemalloc with dhat
```

### Editor Configuration

For VS Code / code-server, install these extensions:

- `rust-lang.rust-analyzer` — Language server
- `tamasfe.even-better-toml` — TOML file support
- `vadimcn.vscode-lldb` — Native debugger support
- `serayuzgur.crates` — Crate dependency management

---

## Running Individual Crates

The workspace currently does not produce standalone binaries. To run a crate's tests or examples:

```bash
# Run core crate tests
cargo test -p ai-os-core

# Run runtime crate tests
cargo test -p ai-os-runtime

# Run a specific test within a crate
cargo test -p ai-os-core test_event_dispatch

# Run benchmarks for a crate
cargo bench -p ai-os-runtime
```

### Adding a Binary Target

To create a runnable binary for a crate, add a `src/main.rs` file:

```rust
// core/src/main.rs (when created)
use ai_os_core::bootstrap::PlatformBuilder;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let app = PlatformBuilder::new().build().await?;
    app.run_until_signal().await?;
    Ok(())
}
```

Then run:

```bash
cargo run -p ai-os-core
```

---

## Hot-Reload Workflow

For rapid iteration during development, use `cargo watch`:

```bash
# Re-run tests on changes
cargo watch -x "test -p ai-os-core"

# Re-build on changes
cargo watch -x "build -p ai-os-runtime"

# Check compilation on changes (fastest feedback)
cargo watch -x "check -p ai-os-core"

# Run clippy on changes
cargo watch -x "clippy -p ai-os-core -- -D warnings"
```

### Watch Multiple Crates

```bash
cargo watch -w core/ -w runtime/ -x "test --workspace"
```

---

## Logging Setup

The project uses the `tracing` crate for structured logging.

### Environment Variable Configuration

```bash
# Default level
export RUST_LOG=info

# Module-specific levels
export RUST_LOG=ai_os_core=debug,ai_os_runtime=info

# Full trace for specific module
export RUST_LOG=ai_os_core::events=trace

# JSON output (for log aggregation)
export RUST_LOG_FORMAT=json

# Enable all logging
export RUST_LOG=trace
```

### In-Code Logging

```rust
use tracing::{info, debug, warn, error, trace, span, Level};

// Structured fields
info!(
    event_type = "task.completed",
    task_id = %task.id(),
    duration_ms = duration.as_millis(),
    "task completed successfully"
);

// Scope-based spans
let span = span!(Level::INFO, "event_processing", event_id = %event.id());
let _guard = span.enter();
// ... processing happens here
```

### Logging in Tests

```bash
# Show log output during tests
RUST_LOG=debug cargo test -- --nocapture

# Or use the tracing-test crate for per-test span introspection
```

---

## Debugging Tips

### Using println! (Last Resort)

```rust
eprintln!("DEBUG: state = {:?}", state);

// For complex types with Debug
dbg!(&state);
```

### Using tracing for Debugging

Add trace logging in suspicious areas:

```rust
trace!(?event, "received event for processing");
// or
debug!(event_type = %event.event_type(), "processing event");
```

### Inspecting Async State

```bash
# Set Tokio console for task inspection
RUSTFLAGS="--cfg tokio_unstable" cargo run
# Then use tokio-console to connect
cargo install tokio-console
tokio-console
```

### Debugging Tests

```bash
# Run a single test with full output
cargo test test_name -- --nocapture

# Run tests with debug output
RUST_LOG=debug cargo test test_name -- --nocapture

# Run tests in release mode (faster, less debug info)
cargo test --release
```

### GDB / LLDB

```bash
# Build with debug symbols (default for debug builds)
cargo build

# Run under LLDB
lldb target/debug/ai-os-core -- <args>

# Or use rust-lldb (shipped with Rust)
rust-lldb target/debug/ai-os-core -- <args>
```

---

## Profiling

### CPU Profiling with perf

```bash
# Build with frame pointers
RUSTFLAGS="-C force-frame-pointers=yes" cargo build --release

# Profile a test
sudo perf record --call-graph dwarf target/release/examples/my_example
sudo perf report

# Or profile a whole test suite
sudo perf record --call-graph dwarf cargo test --release
sudo perf report
```

### Flamegraph Generation

```bash
# Build the binary
cargo build --release

# Generate flamegraph
cargo flamegraph --bin=ai-os-core -- <args>

# For tests
cargo flamegraph --test integration_test_name

# Open the SVG
# The output is flamegraph.svg
```

### Criterion Benchmarks

```bash
# Run all benchmarks
cargo bench --workspace

# Run specific benchmark
cargo bench -p ai-os-runtime scheduler_bench

# Compare against baseline
cargo bench -- --baseline main
```

See the [testing guide](testing.md#benchmark-tests) for benchmark details.

### Heap Profiling

```rust
// Enable dhat heap profiling in your binary
// In Cargo.toml:
// [profile.dev]
// debug = 1
//
// main.rs:
#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

fn main() {
    #[cfg(feature = "dhat-heap")]
    let _profiler = dhat::Profiler::new_heap();

    // ... rest of the program
}
```

Run:

```bash
cargo run --features dhat-heap
# Output: dhat-heap.json — open with dhat-viewer
```

---

## Common Workflows

### Workflow: Feature Development

```bash
# 1. Create a branch
git checkout -b feature/my-feature

# 2. Run the existing tests
cargo test --workspace

# 3. Develop incrementally
cargo watch -x "test -p ai-os-core"

# 4. Run clippy before commit
cargo clippy --workspace --all-targets -- -D warnings

# 5. Commit with conventional message
git commit -m "feat(runtime/scheduler): add priority queue"

# 6. Push and create PR
git push origin feature/my-feature
```

### Workflow: Debugging a Test Failure

```bash
# 1. Find the failing test
cargo test 2>&1 | grep FAILED

# 2. Run with full output
RUST_LOG=trace cargo test failing_test -- --nocapture

# 3. If timing-related, run multiple times
for i in $(seq 10); do cargo test flaky_test; done

# 4. If the test is async, check for deadlocks
# (Tokio console or less frequently: increase timeouts)
```

### Workflow: Dependency Audit

```bash
# 1. Check for known vulnerabilities
cargo audit

# 2. Check for outdated dependencies
cargo outdated

# 3. View transitive dependencies
cargo tree -i tokio

# 4. Update specific dependency
cargo update -p tokio --precise 1.35.0
```

### Workflow: Running the Platform

Once binary targets are added:

```bash
# 1. Build in release mode
cargo build --release

# 2. Configure minimal environment
export RUST_LOG=info

# 3. Run with a sample configuration
./target/release/ai-os-core --config ./configs/default.toml

# 4. Check health
curl http://localhost:9090/health
```

---

## Environment Variables Reference

| Variable | Default | Description |
|---|---|---|
| `RUST_LOG` | `info` | Tracing/logging level filter |
| `RUST_LOG_FORMAT` | `default` | Log output format (`default`, `json`) |
| `AI_OS_CONFIG_DIR` | `/etc/ai-os` | Configuration file directory |
| `AI_OS_DATA_DIR` | `/var/lib/ai-os` | Data storage directory |
| `AI_OS_LOG_DIR` | `/var/log/ai-os` | Log file directory |
| `RUST_BACKTRACE` | `0` | Set to `1` or `full` for backtraces on panics |
| `TOKIO_CONSOLE_BIND` | (unset) | Bind address for Tokio console |
| `SCCACHE_DIR` | `~/.cache/sccache` | sccache cache directory |

---

## See Also

- [Build Guide](build.md) — Building the project and troubleshooting.
- [Testing Guide](testing.md) — Running and writing tests.
- [Coding Standards](coding-standards.md) — Rust code conventions.
- [Project Structure](project-structure.md) — Directory layout and crate responsibilities.
