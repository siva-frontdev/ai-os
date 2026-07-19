# Build Guide

## Prerequisites

### Rust Toolchain

Install the Rust toolchain via `rustup`:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

On Arch Linux, you can also install from the official repositories:

```bash
sudo pacman -S rustup
rustup default stable
```

Verify the installation:

```bash
rustc --version   # Must show the latest stable version
cargo --version
```

### System Dependencies

Run the setup script to install all required system packages:

```bash
./scripts/setup.sh
```

Or install manually:

```bash
sudo pacman -S --needed base-devel git openssl pkg-config cmake clang
```

### Additional Tools

The following tools are recommended for the build workflow:

```bash
# Install via cargo
cargo install cargo-watch cargo-audit cargo-tarpaulin cargo-nextest

# Install sccache for build caching
sudo pacman -S sccache
```

---

## Building the Workspace

### Debug Build

```bash
cargo build
```

This builds all workspace members in debug mode. The output is placed in `target/debug/`.

### Release Build

```bash
cargo build --release
```

Release builds apply optimizations (level 3 by default) and strip debug symbols. Output is in `target/release/`. Release builds are significantly faster but take longer to compile.

### Building Specific Crates

```bash
# Build only the core crate
cargo build -p ai-os-core

# Build only the runtime crate
cargo build -p ai-os-runtime
```

### Building All Targets

```bash
cargo build --workspace --all-targets
```

This builds all workspace crates including tests, benchmarks, and examples. Use this before running the full test suite.

---

## Build Flags

### Customizing Optimization

```bash
# Size-optimized release
RUSTFLAGS="-C target-cpu=native" cargo build --release

# Link-time optimization
RUSTFLAGS="-C lto=thin -C embed-bitcode=yes" cargo build --release
```

### Profile Settings

The project defines custom profiles in `Cargo.toml` as needed. The default `[profile.release]` includes:

```toml
[profile.release]
opt-level = 3
lto = "thin"
codegen-units = 1
strip = "symbols"
debug = false
panic = "abort"
```

### Environment Variables

| Variable | Purpose |
|---|---|
| `RUSTFLAGS` | Pass additional flags to rustc |
| `CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_STRIP` | Strip symbols in release |
| `SCCACHE_DIR` | Override sccache cache directory |
| `CARGO_HTTP_TIMEOUT` | Increase HTTP timeout for slow connections |
| `CARGO_NET_RETRY` | Number of retries for network operations |

---

## Feature Flags

The project uses Cargo feature flags sparingly. As of Phase 4, the following features may be present:

```toml
[features]
default = ["std"]
std = []
unstable = []       # Experimental features, not for production
metrics = []        # Enable metrics collection
profiling = []      # Enable profiling instrumentation
```

List available features:

```bash
cargo metadata --no-deps --format-version 1 | jq '.packages[].features'
```

Build with specific features:

```bash
cargo build --features "metrics,profiling"
```

---

## Cross-Compilation

### Prerequisites

Install the cross-compilation target and linker:

```bash
# For aarch64 (ARM64)
rustup target add aarch64-unknown-linux-gnu
sudo pacman -S aarch64-linux-gnu-gcc

# For x86_64 (targeting older glibc)
rustup target add x86_64-unknown-linux-gnu
```

### Configuring the Linker

Set the linker in `.cargo/config.toml`:

```toml
[target.aarch64-unknown-linux-gnu]
linker = "aarch64-linux-gnu-gcc"
```

### Cross-Compile Command

```bash
cargo build --target aarch64-unknown-linux-gnu --release
```

### Using Cross (Containerized Cross-Compilation)

For targets without native toolchain support, use `cross`:

```bash
cargo install cross
cross build --target aarch64-unknown-linux-gnu --release
```

---

## Build Caching with sccache

The project uses `sccache` (Shared Compilation Cache) to accelerate rebuilds across branches and CI jobs.

### Setup

```bash
sudo pacman -S sccache
```

Configure sccache as the Rust compiler wrapper:

```toml
# In .cargo/config.toml
[build]
rustc-wrapper = "sccache"
```

Or set the environment variable:

```bash
export RUSTC_WRAPPER=sccache
```

### Verification

```bash
sccache --show-stats
```

### CI Configuration

In CI, configure sccache with a remote cache backend (S3, GCS, or Redis):

```bash
export SCCACHE_BUCKET=ai-os-build-cache
export SCCACHE_REGION=us-east-1
export RUSTC_WRAPPER=sccache
```

---

## CI Build Process

The CI pipeline runs the following build steps on every push:

### Build Stage

```yaml
# .github/workflows/ci.yml (conceptual)
jobs:
  build:
    runs-on: [self-hosted, linux, x64]
    steps:
      - uses: actions/checkout@v4

      - uses: actions-rust-lang/setup-rust-toolchain@v1
        with:
          toolchain: stable

      - name: Build (debug)
        run: cargo build --workspace --all-targets

      - name: Build (release)
        run: cargo build --workspace --release

      - name: Lint
        run: cargo clippy --workspace --all-targets -- -D warnings

      - name: Format
        run: cargo fmt --all -- --check

      - name: Test
        run: cargo test --workspace

      - name: Audit
        run: cargo audit
```

### CI Artifacts

Release builds in CI produce the following artifacts:

- `target/release/ai-os-core` (if it produces a binary)
- `target/release/ai-os-runtime` (if it produces a binary)
- Any test binaries for integration test suites

---

## Troubleshooting Common Build Errors

### "linker `cc` not found"

Install the required base development packages:

```bash
sudo pacman -S base-devel gcc
```

### "openssl-sys build failed"

Install OpenSSL development headers:

```bash
sudo pacman -S openssl pkg-config
```

### "failed to select a version for `tokio`"

The dependency resolver may need updating. Ensure the workspace resolver is set to "2":

```toml
[workspace]
resolver = "2"
```

Then update the lockfile:

```bash
cargo update
```

### "the `...` binary is not available on the stable channel"

Some Cargo subcommands require the nightly toolchain. Use `cargo +nightly <command>` or install the toolchain:

```bash
rustup toolchain install nightly
```

### "error: failed to run custom build command for `prost-build`"

Install Protocol Buffers compiler:

```bash
sudo pacman -S protobuf
```

### "could not find `ai-os-core` in workspace"

Ensure the workspace `Cargo.toml` includes the crate in `members`:

```toml
[workspace]
members = ["core", "runtime"]
```

### "fatal error: too many open files"

Increase the file descriptor limit:

```bash
ulimit -n 4096
```

### Out of Memory During Build

On memory-constrained systems, reduce parallelism:

```bash
# Limit to 1 codegen unit
CARGO_BUILD_JOBS=2 cargo build

# Or reduce parallel jobs
cargo build --jobs 2
```

---

## Quick Reference

| Task | Command |
|---|---|
| Debug build all | `cargo build` |
| Release build all | `cargo build --release` |
| Build specific crate | `cargo build -p ai-os-core` |
| Build all targets | `cargo build --workspace --all-targets` |
| Clean build artifacts | `cargo clean` |
| Check (no output generation) | `cargo check` |
| Update dependencies | `cargo update` |
| Dependency tree | `cargo tree` |
| Outdated dependencies | `cargo outdated` |
| Build cache stats | `sccache --show-stats` |

---

## See Also

- [Development Guide](development.md) — Setting up the development environment and workflow.
- [CI/CD Pipeline](contributing.md#ci-expectations) — CI build and test pipeline.
- [Testing Guide](testing.md) — Running tests and benchmarks.
