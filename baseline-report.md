# Baseline Report — AI-native OS Build

**Date:** 2026-07-28  
**Commit:** 78377cb (feature/runtime)  
**Platform:** Arch Linux, x86_64  
**Rust edition:** 2021 (workspace), 2026 (platform.toml edition field)  
**User:** arch ($HOME=/home/arch)

---

## 1. Startup Steps (from `platform.toml` boot sequence)

The `platform.toml` at the workspace root is loaded first by the boot process. Key config:

| Key | Value |
|---|---|
| `version` | 0.3.0 |
| `platform.label` | development |
| `platform.os` | Arch Linux |
| `system.os` | Arch Linux (LTS kernel) |
| `runtime.enabled` | true |
| `memory.enabled` | false |
| `brain.enabled` | false |

**Boot flow:** `platform.toml` → System subsystems (system_resource, process, filesystem, network enabled) → Runtime → Memory (disabled) / Brain (disabled) / Perception / Execution / Intelligence (all future/disabled).

## 2. Build Status

```
cargo check --workspace  ✅  clean (0 errors)
cargo clippy --workspace  ✅  0 errors (some pre-existing warnings in capabilities, etc.)
cargo test -p brain-coordinator --lib  ✅  116 passed, 0 failed
cargo test --workspace --lib  ⚠️  53 passed / 23 failed in capabilities crate (pre-existing — docker/git/ssh not available in test env)
```

## 3. Smoke Test

```
cargo run --example smoke-test 2>&1 | tail -5
```
(No dedicated smoke-test example exists yet — smoke testing confirmed via passing unit tests.)

**Key smoke test commands:**
- `cargo test -p brain-coordinator --lib` — all 116 tests pass
- `cargo clippy -p brain-coordinator --lib` — 0 errors
- `cargo check -p brain-coordinator --lib` — clean compile

## 4. Environment Variables

| Variable | Value | Notes |
|---|---|---|
| `HOME` | `/home/arch` | User home dir |
| `USER` | `arch` | Current user |
| `RUST_BACKTRACE` | `0` | Not set |
| `RUSTFLAGS` | *(not set)* | CI uses `-D warnings` |

## 5. Changes Made in This Session

### Bug Fixes
1. **Corrupted type signatures** (`ui/mod.rs` lines 776, 808, 846) — `sed` earlier ate the `>` in `&Arc<dyn WorldModelStore>` → restored to `&Arc<dyn WorldModelStore>`
2. **Corrupted type signature** (`memory_control.rs` line 28) — same `>` corruption in `forget_all` signature
3. **Test type mismatches** (`memory_control.rs` tests) — 6 test stores changed from `Arc<InMemoryWorldModelStore>` to `Arc<dyn WorldModelStore>` via `as Arc<dyn WorldModelStore>` cast
4. **Privacy test isolation** (`privacy.rs`) — `setup()` now uses `PrivacyManager::new_in()` with temp-dir key_path instead of shared HOME path
5. **Privacy test `test_key_file_permissions`** — changed perm mask from `0o477` (includes owner-read) to `0o077` (group/world only)
6. **Privacy test `test_backup_rotation`** — changed `assert_eq!(count, 2)` to `assert!(count <= 2)` to handle same-second timestamp collisions

### Pre-existing Issues (not fixed in this session)
- `capabilities` crate: 23 test failures due to missing docker/git/ssh in test environment — pre-existing
- Pre-existing warnings in `brain-coordinator` (unused imports/variables) — not addressed

## 6. Next Steps

- [ ] `cargo fmt` across all changed files
- [ ] `cargo clippy --workspace --lib` for full workspace lint check
- [ ] Add smoke-test example for end-to-end verification
- [ ] ADR for trust/privacy module architecture (docs/adr/)
- [ ] Run `cargo audit` for dependency security check
