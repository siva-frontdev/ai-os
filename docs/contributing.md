# Contributing to AI-OS

Thank you for your interest in contributing to the AI-native OS project. This guide documents the contribution workflow, conventions, and expectations.

For setup instructions, see the [development guide](development.md). For design context, read the [design principles](principles.md).

---

## Code of Conduct

All contributors must adhere to the Rust standard of conduct: be respectful, constructive, and professional. Harassment, discrimination, and personal attacks are not tolerated.

---

## How to Report Bugs

### Before Reporting

1. Search the issue tracker for existing reports of the same bug.
2. Verify the bug exists on the latest commit of `main`.
3. Check if the bug is a known limitation documented in the project.

### Bug Report Requirements

A good bug report includes:

- **Environment**: Rust version (`rustc --version`), OS version, AI-OS commit hash.
- **Expected behavior**: What should happen.
- **Actual behavior**: What actually happens, including error messages.
- **Steps to reproduce**: Minimal, reproducible steps. Include code or test cases.
- **Impact**: How the bug affects your work (blocking, minor, cosmetic).
- **Logs**: Relevant log output at `RUST_LOG=debug` or `trace` level.

Use the GitHub issue template for bug reports. Label the issue with `bug`.

### Security Vulnerabilities

Do not report security vulnerabilities in public issues. Follow the [security policy](security.md#vulnerability-reporting-process) for responsible disclosure.

---

## Feature Request Process

1. Open a feature request issue using the feature request template.
2. Clearly describe the problem you want to solve, not just the solution you propose.
3. Explain how the feature aligns with the [project roadmap](roadmap.md) and [design principles](principles.md).
4. Include examples of how the feature would be used.

Feature requests are reviewed by the core team. Small features may be approved immediately; large features require an Architecture Decision Record (ADR) before implementation begins.

---

## Pull Request Workflow

### Step 1: Create a Branch

Branch from `main` using a descriptive name following the branch naming conventions:

| Prefix | Purpose | Example |
|---|---|---|
| `feature/` | New feature | `feature/scheduler-priority-queue` |
| `fix/` | Bug fix | `fix/event-bus-memory-leak` |
| `docs/` | Documentation | `docs/api-reference-update` |
| `refactor/` | Code refactoring | `refactor/error-type-consolidation` |
| `test/` | Test additions | `test/scheduler-property-tests` |
| `chore/` | Maintenance | `chore/update-tokio-1.35` |
| `perf/` | Performance | `perf/async-channel-optimization` |

```bash
git checkout main
git pull origin main
git checkout -b feature/my-feature
```

### Step 2: Develop

Follow the [coding standards](coding-standards.md) during development. Commit frequently with [conventional commit messages](#commit-message-format).

### Step 3: Test

Before opening a PR:

```bash
# Run all tests
cargo test --workspace

# Run clippy (must pass with zero warnings)
cargo clippy --workspace --all-targets -- -D warnings

# Check formatting
cargo fmt --all -- --check

# Check documentation builds
cargo doc --workspace --no-deps

# Run audit
cargo audit
```

See the [testing guide](testing.md) for detailed testing expectations.

### Step 4: Open the Pull Request

1. Push your branch to GitHub: `git push origin feature/my-feature`.
2. Open a pull request against `main`.
3. Fill out the PR template with:
   - **Summary**: One-paragraph description of what the PR does.
   - **Related issues**: `Closes #123` or `Related to #456`.
   - **Checklist**: Confirm tests pass, docs updated, no breaking changes.
   - **Manual testing notes**: Steps the reviewer can follow to verify.
4. Request review from the relevant code owners.

### Step 5: Code Review

During review:
- Address all reviewer comments. Each conversation must be resolved before merge.
- Push additional commits to address feedback. Do not squash or force-push until review is complete.
- Keep the scope of the PR focused. If new issues are discovered, file them separately.
- The reviewer(s) will approve when the code meets the standards.

### Step 6: Merge

1. Squash commits into a single commit with a descriptive message.
2. Merge via the GitHub `Squash and merge` button (preferred) or `Rebase and merge`.
3. Delete the feature branch after merge.

---

## Commit Message Format

The project uses [Conventional Commits](https://www.conventionalcommits.org/) specification:

```
<type>(<scope>): <description>

[optional body]

[optional footer(s)]
```

### Types

| Type | Usage |
|---|---|
| `feat` | A new feature |
| `fix` | A bug fix |
| `docs` | Documentation only changes |
| `style` | Changes that do not affect the meaning of code (formatting) |
| `refactor` | A code change that neither fixes a bug nor adds a feature |
| `perf` | A code change that improves performance |
| `test` | Adding missing or correcting existing tests |
| `chore` | Changes to the build process, tooling, or dependencies |
| `ci` | Changes to CI configuration |
| `revert` | Reverts a previous commit |

### Scope

The scope identifies the crate or module affected:

```
feat(runtime/scheduler): add priority-based task queue
fix(core/events): prevent double-free on channel close
docs(core): update event bus API documentation
```

### Examples

```
feat(runtime/scheduler): add work-stealing task dispatch

Implement a work-stealing scheduler that balances load across
worker pools. Each worker maintains a local queue and steals
from others when idle.

Closes #127
```

```
fix(core/events): prevent panic on empty subscriber list

Return `CoreError::NoHandler` instead of panicking when an
event is dispatched with zero subscribers.

Closes #134
```

### Body and Footer

- The body explains the *why* and *what*, not the *how*.
- Use `Closes #N`, `Fixes #N`, or `Related to #N` in the footer to link issues.
- Breaking changes are marked with `BREAKING CHANGE:` in the footer or `!` after the type/scope.

---

## Code Review Process

### Reviewer Expectations

1. Review within 2 business days. If unable to review in that time, communicate expectations.
2. Verify the code follows the [coding standards](coding-standards.md).
3. Verify tests are adequate and passing.
4. Verify documentation is updated.
5. Verify no regressions in performance or security.
6. Leave constructive, specific feedback. Explain the *why* behind suggestions.

### Author Expectations

1. Respond to all review comments within 2 business days.
2. Explain rationale when disagreeing with a suggestion.
3. Keep PRs small and focused. A PR should change one thing. Large PRs may be rejected for splitting.
4. Re-request review after addressing all comments.

### Required Approvals

- All PRs require at least one approval from a code owner of the affected crates.
- Changes to `docs/` and `architecture/` require approval from the technical writing lead.
- Changes to `core/` require approval from at least two core team members.
- Changes to CI or build configuration require approval from the infrastructure lead.

---

## CI Expectations

The CI pipeline runs the following checks on every push:

1. **Build**: `cargo build --workspace --all-targets` (debug and release).
2. **Format**: `cargo fmt --all -- --check`.
3. **Lint**: `cargo clippy --workspace --all-targets -- -D warnings`.
4. **Test**: `cargo test --workspace`.
5. **Doc**: `cargo doc --workspace --no-deps` (checks for broken links).
6. **Audit**: `cargo audit` for dependency vulnerabilities.
7. **Coverage**: Uploads coverage report (for `main` branch only).

All CI checks must pass before merging. CI failures on the `main` branch must be treated as P0 incidents.

---

## Testing Requirements Before PR

Before opening a pull request, the contributor must:

1. Run the full test suite: `cargo test --workspace`.
2. Run clippy with zero warnings: `cargo clippy --workspace --all-targets -- -D warnings`.
3. Ensure the PR includes tests for new functionality (unit, integration, and property-based where appropriate).
4. Ensure the PR includes a regression test for any fixed bug.
5. For performance-sensitive changes, include Criterion benchmarks and compare against `main`.

---

## Documentation Requirements

A PR is not complete until documentation is updated:

1. **rustdoc**: All new public API items have rustdoc comments.
2. **This file**: Update if contribution workflows change.
3. **Architecture docs**: Update in `docs/` if the change affects the architecture.
4. **Glossary**: Update `docs/glossary.md` if new terminology is introduced.
5. **README**: Update if the change affects the top-level project description or quick start.

See the [coding standards](coding-standards.md#documentation-standards) for rustdoc requirements.

---

## Branch Management

### Branch Naming

Branches follow the convention: `<type>/<short-description>`.

Examples:
- `feature/scheduler-priority-queue`
- `fix/event-bus-deadlock`
- `docs/api-lifecycle-diagrams`

### Branch Lifecycle

- Feature branches are short-lived (days, not weeks). Long-running branches should be rebased onto `main` regularly.
- After merge, delete the branch both locally and on the remote.
- The `main` branch is always releasable. Do not push directly to `main`.

---

## Seeking Help

If you need help at any stage:

1. **Issue tracker**: Use GitHub issues for technical questions and bug reports.
2. **GitHub Discussions**: Use the Discussions tab for design discussions, feature proposals, and general questions.
3. **Code review comments**: Leave questions on the relevant PR lines.
4. **Documentation**: Check the [docs/](project-structure.md#docs-organization) directory first. If documentation is unclear, file a documentation issue.

---

## Recognition

Contributors are recognized through:
- GitHub contribution graph and commit history.
- Release notes and changelog acknowledgments.
- Core team membership for sustained, high-quality contributions.

---

## Appendix: Quick Reference

### Before Every Commit

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings  # fix all warnings
cargo test --workspace                                   # ensure tests pass
cargo doc --workspace --no-deps                          # no broken doc links
```

### Before Every PR

```bash
git pull origin main --rebase
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
cargo doc --workspace --no-deps
git push origin feature/my-feature
```

### PR Checklist

- [ ] Tests pass (`cargo test --workspace`)
- [ ] Clippy passes with zero warnings
- [ ] Formatting is correct (`cargo fmt --all -- --check`)
- [ ] Documentation is updated (rustdoc, this file, architecture docs)
- [ ] Commit messages follow Conventional Commits
- [ ] Branch is up to date with `main`
- [ ] No merge conflicts
- [ ] CI passes
