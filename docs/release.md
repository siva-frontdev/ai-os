# Release Process

## Version Numbering

The project follows [Semantic Versioning 2.0.0](https://semver.org/):

```
MAJOR.MINOR.PATCH
```

| Component | When to Increment | Example |
|---|---|---|
| MAJOR | Incompatible API or behavioral changes | `1.0.0` to `2.0.0` |
| MINOR | Backward-compatible new functionality | `1.0.0` to `1.1.0` |
| PATCH | Backward-compatible bug fixes | `1.0.0` to `1.0.1` |

### Pre-Release Suffixes

Pre-release versions use suffixes for development and testing:

```
1.0.0-alpha.1     -- Early feature iteration
1.0.0-beta.1      -- Feature-complete, testing
1.0.0-rc.1        -- Release candidate, final validation
```

### Version Source of Truth

The canonical version is defined in the workspace `Cargo.toml`:

```toml
[workspace.package]
version = "0.1.0"
```

All workspace crates inherit this version via `version.workspace = true`. Individual crate versions are not maintained separately.

---

## Pre-Release Checklist

Before cutting a release, the release manager must verify each item:

### Code Quality

- [ ] All CI checks pass on `main` (build, lint, test, audit, format).
- [ ] No open `P0` or `P1` bugs targeting this release.
- [ ] All PRs in the release milestone are merged.
- [ ] Code coverage is at or above the target threshold (80%).
- [ ] All flaky tests are resolved or documented.

### Documentation

- [ ] `CHANGELOG.md` is up to date with all changes since the last release.
- [ ] `docs/` files are reviewed for accuracy.
- [ ] `README.md` quick start instructions are current.
- [ ] API documentation builds without errors: `cargo doc --workspace --no-deps`.

### Security

- [ ] `cargo audit` passes with no vulnerabilities.
- [ ] No unaddressed security advisories.
- [ ] Dependency licenses are reviewed (if publishing to crates.io).

### Operations

- [ ] Configuration files in `configs/` are current.
- [ ] Systemd service files are current (see the [deployment guide](deployment.md)).
- [ ] Backup of the current production state is verified (if applicable).

### Release Artifacts

- [ ] Release binaries build cleanly: `cargo build --release --workspace`.
- [ ] Release binaries are tested on the target platform (AWS EC2, Arch Linux).
- [ ] Cross-compilation targets build cleanly (if applicable).

---

## Changelog Generation

The changelog is maintained in `CHANGELOG.md` at the project root. It follows the [Keep a Changelog](https://keepachangelog.com/) format.

### Structure

```markdown
# Changelog

## [0.2.0] - 2026-07-19

### Added
- feat(runtime/scheduler): priority-based task queue (#127)
- feat(core/events): wildcard subscription patterns (#131)

### Changed
- refactor(runtime/permission): use builder pattern for permission checks (#128)

### Fixed
- fix(core/health): race condition in concurrent health checks (#130)

### Security
- deps: update tokio to 1.35.0 (#132)

## [0.1.0] - 2026-06-01

### Added
- Initial release: Core Platform (Phase 2) and Runtime Platform (Phase 3)
```

### Generation Process

1. Aggregate all merged PRs since the last release tag.
2. Categorize by type using Conventional Commits prefixes (`feat`, `fix`, `docs`, `refactor`, `perf`, `test`, `chore`, `ci`, `security`).
3. Write a human-readable summary for each change, referencing the PR number.
4. Review for completeness and accuracy.
5. Commit the updated changelog to `main` before tagging.

### Tooling

Changelog generation can be automated with `git-cliff`:

```bash
# Install
cargo install git-cliff

# Generate changelog since last tag
git-cliff --unreleased --tag 0.2.0

# Update CHANGELOG.md
git-cliff --output CHANGELOG.md
```

---

## Tagging

### Creating a Tag

Once the changelog is committed and the pre-release checklist is complete:

```bash
# Ensure we're on the release commit
git checkout main
git pull origin main

# Create an annotated tag
git tag -a v0.2.0 -m "Release v0.2.0"

# Push the tag
git push origin v0.2.0
```

### Tag Naming Convention

Tags follow the pattern `v<version>`:

```
v0.1.0
v0.2.0
v1.0.0
v1.0.0-rc.1
```

### Tag Signing

Tags should be signed with a GPG key:

```bash
git tag -s v0.2.0 -m "Release v0.2.0"
```

Configure Git to use your signing key:

```bash
git config user.signingkey <KEY_ID>
git config tag.gpgsign true
```

---

## Crate Publishing (crates.io)

If publishing workspace crates to crates.io, follow these steps:

```bash
# Verify the package builds and passes all checks
cargo publish -p ai-os-core --dry-run

# Publish in dependency order (core first, then runtime)
cargo publish -p ai-os-core
cargo publish -p ai-os-runtime
```

### Pre-Publish Checks

- [ ] Package metadata in `Cargo.toml` is complete (description, license, repository, keywords, categories).
- [ ] No secrets or sensitive files are included (check `Cargo.toml` `exclude` or `include` directives).
- [ ] README.md renders correctly on crates.io.
- [ ] Documentation links are valid.
- [ ] `cargo publish --dry-run` completes successfully.

### Post-Publish Verification

```bash
# Verify the published crate
cargo search ai-os-core
# Or use the crates.io web interface

# Verify documentation is published
# https://docs.rs/ai-os-core/<version>/
```

---

## GitHub Release Creation

After the tag is pushed, create a GitHub Release:

### Using the GitHub Web Interface

1. Navigate to the repository on GitHub.
2. Click "Releases" in the right sidebar.
3. Click "Draft a new release".
4. Select the tag (`v0.2.0`).
5. Set the title to the tag name (`v0.2.0`).
6. Write release notes summarizing the changelog.
7. Attach any release artifacts (binaries, checksums).
8. Mark as "Latest release" (unless it is a pre-release).
9. Publish the release.

### Using the GitHub CLI

```bash
# Create the release
gh release create v0.2.0 \
    --title "v0.2.0" \
    --notes "Release notes here..." \
    --latest

# With artifacts
gh release create v0.2.0 \
    target/release/ai-os-core \
    target/release/ai-os-runtime \
    --title "v0.2.0"

# Pre-release
gh release create v0.2.0-rc.1 \
    --prerelease \
    --title "v0.2.0-rc.1"
```

### Release Artifacts

Attach the following to each GitHub Release:

- `ai-os-core-x86_64-linux` — Core platform binary (when available)
- `ai-os-runtime-x86_64-linux` — Runtime platform binary (when available)
- `checksums.txt` — SHA-256 checksums of all artifacts
- `configs.tar.gz` — Default configuration files

Generate checksums:

```bash
sha256sum target/release/* > checksums.txt
```

---

## Post-Release Verification

After the release is published:

1. **Verify the tag**: `git tag -l 'v*'`
2. **Verify the release**: Check the GitHub Releases page.
3. **Verify the crates.io entry**: If published, check the crates.io page.
4. **Verify documentation**: Check docs.rs (if published).
5. **Deploy to staging**: Test the release in a staging environment.
6. **Run integration smoke tests**: `cargo test --workspace --test '*'`
7. **Verify metrics**: Check that the release deployed successfully in monitoring.

---

## Hotfix Process

A hotfix is a patch release for a critical bug in a released version that cannot wait for the next regular release.

### Criteria for a Hotfix

- P0 severity: production outage, data loss, or security vulnerability.
- The fix is minimal and low-risk.
- The fix has been reviewed and tested.

### Hotfix Steps

```bash
# 1. Create a hotfix branch from the release tag
git checkout -b hotfix/v0.2.1 v0.2.0

# 2. Apply the fix
git cherry-pick <commit-hash>

# 3. Bump the patch version
# Update version in Cargo.toml
# Update CHANGELOG.md with the hotfix entry

git commit -m "chore: bump version to v0.2.1"

# 4. Tag the hotfix
git tag -a v0.2.1 -m "Hotfix v0.2.1"
git push origin v0.2.1

# 5. Create a GitHub Release
gh release create v0.2.1 --title "v0.2.1" --notes "Hotfix for ..."

# 6. Merge the hotfix back to main
git checkout main
git merge hotfix/v0.2.1
git push origin main
```

### Hotfix Versioning

Hotfixes increment the PATCH version: `0.2.0` -> `0.2.1`.

---

## Release Cadence

| Phase | Cadence | Description |
|---|---|---|
| Development (pre-1.0) | Every 2-4 weeks | Minor version increments for features and fixes |
| Stable (1.0+) | Every 4-8 weeks | Regular feature releases |
| Patch | As needed | Critical bug fixes and security patches |
| Pre-release | Before each minor | Alphas, betas, release candidates |

---

## Quick Reference

| Task | Command |
|---|---|
| Update version | Edit `Cargo.toml` workspace version field |
| Update changelog | Edit `CHANGELOG.md` |
| Create tag | `git tag -a v0.2.0 -m "Release v0.2.0"` |
| Push tag | `git push origin v0.2.0` |
| Publish crate | `cargo publish -p ai-os-core` |
| Create GitHub release | `gh release create v0.2.0` |
| Start hotfix | `git checkout -b hotfix/v0.2.1 v0.2.0` |

---

## See Also

- [Contributing Guide](contributing.md) — PR workflow and CI expectations.
- [Build Guide](build.md) — Release build configuration.
- [Deployment Guide](deployment.md) — Production deployment and rollback.
- [Security Guide](security.md) — Security vulnerability handling.
