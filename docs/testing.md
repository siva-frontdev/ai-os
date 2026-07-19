# Testing Guide

## Test Philosophy

Testing is integral to the AI-native OS development process, not a separate phase. The project follows these principles derived from the [design principles](principles.md):

1. **Test-first**: Tests are written before or concurrently with production code (Principle 11).
2. **Every bug includes a regression test**: No bug fix is complete without a test that would have caught it.
3. **Multiple test levels**: Unit tests, integration tests, property-based tests, and benchmarks each serve distinct purposes.
4. **Tests are code**: Tests follow the same coding standards as production code.
5. **Deterministic**: Tests must be repeatable. Flaky tests are treated as bugs.

---

## Test Levels

### Unit Tests

**Purpose**: Verify individual functions, methods, and modules in isolation.

**Location**: Co-located with the code they test, in a `#[cfg(test)] mod tests` block:

```
core/src/events/
  mod.rs          -- Implementation
  tests.rs        -- Unit tests module (or inline in mod.rs)
```

**Conventions**:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_dispatch_event() {
        let mut bus = EventBus::new();
        let mut rx = bus.subscribe("test.event");

        bus.dispatch(Event::new("test.event", "payload")).await;

        let event = rx.recv().await.unwrap();
        assert_eq!(event.event_type(), "test.event");
    }

    #[tokio::test]
    async fn test_dispatch_no_handler() {
        let bus = EventBus::new();
        let result = bus.dispatch(Event::new("unknown", "payload")).await;
        assert!(result.is_err());
    }
}
```

**Requirements**:
- Every public function must have at least one happy-path and one error-path unit test.
- Use `#[tokio::test]` for async tests.
- Test names follow the pattern `test_<function>_<condition>`.
- Isolate tests from external dependencies (mock the EventBus, filesystem, network).

### Integration Tests

**Purpose**: Verify that modules and services work together correctly.

**Location**: `tests/` at each crate root:

```
core/tests/
  event_bus_integration.rs  -- EventBus multi-module interaction
  lifecycle_integration.rs  -- Service lifecycle state transitions

runtime/tests/
  scheduler_integration.rs  -- Scheduler + worker interaction
  supervisor_integration.rs -- Supervisor + health check interaction
```

**Conventions**:

```rust
// tests/common/mod.rs -- Shared test setup
pub fn create_test_platform() -> Platform {
    PlatformBuilder::new()
        .with_event_bus(EventBus::new())
        .with_logger(Logger::new())
        .build()
        .unwrap()
}

// tests/event_bus_integration.rs
mod common;

#[tokio::test]
async fn test_event_routing_between_modules() {
    let platform = common::create_test_platform();

    // Register two services that communicate via events
    platform.register(ProducerService::new()).await;
    platform.register(ConsumerService::new()).await;

    // Producer dispatches event; Consumer receives and processes
    platform.start().await;

    // Assert consumer state updated
    // ...
}
```

**Requirements**:
- Integration tests are separate binaries, each in their own file in `tests/`.
- Shared setup code lives in `tests/common/mod.rs`.
- Integration tests may use `anyhow::Result` for convenience error handling.
- Each integration test file tests one scenario end-to-end.

### Property-Based Tests

**Purpose**: Verify that invariants hold for a wide range of inputs.

**Location**: Co-located with unit tests, separated by a module comment:

```rust
#[cfg(test)]
mod tests {
    // ... unit tests ...

    mod prop {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn test_scheduling_order_invariant(
                priorities in prop::collection::vec(0u8..255, 1..100),
            ) {
                let mut queue = PriorityQueue::new();
                for p in &priorities {
                    queue.push(Task::with_priority(*p));
                }
                let mut scheduled = Vec::new();
                while let Some(task) = queue.pop() {
                    scheduled.push(task.priority());
                }
                // Invariant: scheduled priorities are non-increasing
                for w in scheduled.windows(2) {
                    assert!(w[0] >= w[1], "priority order violated");
                }
            }
        }
    }
}
```

**When to use property-based tests**:
- Scheduling algorithms and queue invariants.
- Permission evaluation: "no operation is both allowed and denied."
- State machine transitions: "every valid state has a defined transition for each input."
- Serialization round-trips: "deserialize(serialize(x)) == x."
- Any function with a large input space where edge cases are hard to enumerate.

### Benchmark Tests

**Purpose**: Measure performance and detect regressions.

**Location**: `benches/` at each crate root:

```
runtime/benches/
  runtime_bench.rs
```

**Conventions** (using Criterion):

```rust
use criterion::{criterion_group, criterion_main, Criterion};

fn scheduler_bench(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("scheduler_dispatch_1000_tasks", |b| {
        b.to_async(&runtime).iter(|| async {
            let mut scheduler = Scheduler::new();
            for i in 0..1000 {
                scheduler.submit(Task::new(format!("task-{}", i))).await;
            }
            scheduler.drain().await;
        });
    });
}

criterion_group!(benches, scheduler_bench);
criterion_main!(benches);
```

**Requirements**:
- Run benchmarks before and after performance-sensitive changes.
- Compare against the `main` branch baseline.
- Store historical baseline data in `target/criterion/` (not committed).
- Document benchmark names clearly: `scheduler_dispatch_N_tasks`.

---

## Running Tests

### All Tests

```bash
# Run all tests across the workspace
cargo test --workspace

# Run all tests with output
cargo test --workspace -- --nocapture
```

### Single Crate

```bash
# Core crate
cargo test -p ai-os-core

# Runtime crate
cargo test -p ai-os-runtime
```

### Specific Test

```bash
# By name (partial match)
cargo test test_dispatch_event

# Exact match
cargo test -- test_dispatch_event

# Module path
cargo test events::tests::
```

### Filtered Test Run

```bash
# Run only integration tests (in tests/ directory)
cargo test --test event_bus_integration

# Run only unit tests (skip integration tests)
cargo test --lib
```

### Using cargo-nextest

For faster test execution with better reporting:

```bash
cargo nextest run --workspace

# With JUnit output for CI
cargo nextest run --workspace --junit output.xml

# Retry flaky tests
cargo nextest run --workspace --retries 2
```

### Running Benchmarks

```bash
# All benchmarks
cargo bench --workspace

# Specific benchmark
cargo bench -p ai-os-runtime scheduler_bench

# Compare against baseline
cargo bench -- --baseline main

# Save a new baseline
cargo bench -- --save-baseline current
```

---

## CI Test Gates

The CI pipeline enforces the following test gates on every push:

| Gate | Command | Failure Threshold |
|---|---|---|
| Unit tests | `cargo test --workspace --lib` | Any failure |
| Integration tests | `cargo test --workspace --test '*'` | Any failure |
| Clippy | `cargo clippy --workspace --all-targets -- -D warnings` | Any warning |
| Format | `cargo fmt --all -- --check` | Unformatted files |
| Doc tests | `cargo test --workspace --doc` | Any failure |
| Audit | `cargo audit` | Any vulnerability |

All gates must pass before a PR can be merged. The CI pipeline runs on every push to any branch and on every pull request targeting `main`.

---

## Flaky Test Policy

A flaky test is a test that passes and fails without any code change.

### Detection

- A test that fails intermittently in CI (observed 3+ times without a related code change) is classified as flaky.
- Tests that fail only in CI (never locally) should first be investigated for environment differences.

### Resolution

1. File an issue labeled `flaky-test` with the test name and failure pattern.
2. If the test is low-value, consider removing or replacing it with a more stable version.
3. If the test is high-value, diagnose the root cause (timing dependency, shared state leak, async race) and fix.
4. While the issue is open, exclude the test from CI by adding it to the flaky test ignore list:

```toml
# In .cargo/config.toml or CI configuration
# Flaky tests to skip in CI
[[test]]
name = "flaky_test_name"
ignore = true
```

### Prevention

- Avoid `sleep` in tests. Use `tokio::time::timeout` with event-driven synchronization.
- Do not depend on wall-clock time for correctness assertions.
- Use deterministic seeds for random generators.
- Isolate tests from shared global state.

---

## Code Coverage

### Local Coverage (tarpaulin)

```bash
# Install
cargo install cargo-tarpaulin

# Run coverage on the workspace
cargo tarpaulin --workspace --out Html

# Run coverage on a specific crate
cargo tarpaulin -p ai-os-core --out Xml

# Output: tarpaulin-report.html or cobertura.xml
```

### CI Coverage

Coverage is computed on every push to `main` using `cargo-tarpaulin` or `grcov`:

```bash
# grcov approach (requires nightly)
rustup toolchain install nightly
cargo +nightly test --workspace
grcov . --binary-path target/debug -s . -t lcov --branch --ignore-not-existing -o coverage.lcov
```

Coverage reports are uploaded to a coverage dashboard. The current coverage target is 80% line coverage for the `core/` and `runtime/` crates.

---

## Test Directory Structure Summary

```
core/
  src/
    events/
      mod.rs          -- Implementation
      tests.rs        -- Unit tests
    lifecycle/
      mod.rs
      tests.rs
  tests/
    event_bus_integration.rs
    lifecycle_integration.rs
    common/
      mod.rs          -- Shared test helpers

runtime/
  src/
    scheduler/
      mod.rs
      tests.rs
    supervisor/
      mod.rs
      tests.rs
  tests/
    scheduler_integration.rs
    supervisor_integration.rs
    common/
      mod.rs
  benches/
    runtime_bench.rs
```

---

## Writing Tests Checklist

Before submitting a PR, verify:

- [ ] Does every new public function have unit tests?
- [ ] Do error paths have test coverage?
- [ ] Are there integration tests for the new module's interactions?
- [ ] For algorithms: are there property-based tests for invariants?
- [ ] For performance-sensitive changes: are there Criterion benchmarks?
- [ ] Do all tests pass locally? (`cargo test --workspace`)
- [ ] Are tests free of flaky patterns (sleep, time-dependent assertions)?
- [ ] Is the test naming consistent with conventions? (`test_function_condition`)
- [ ] Are async tests using `#[tokio::test]`?
- [ ] Do integration tests use shared setup from `tests/common/`?

---

## See Also

- [Coding Standards](coding-standards.md#testing-requirements) — Test conventions in code.
- [Contributing Guide](contributing.md) — PR testing expectations.
- [Build Guide](build.md) — CI build and test setup.
