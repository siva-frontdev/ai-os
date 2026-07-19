# Security Guide

## Introduction

Security is a foundational property of AI-native OS, not a feature to be added later. This document describes the security model, threat considerations, and operational security practices for the project.

Security decisions are guided by [Principle 4: Security by Default](principles.md#principle-4-security-by-default) — the default configuration is secure, permissions are denied by default, and security is enforced at every layer of the architecture.

---

## Threat Model Overview

### Assets

- **Events**: All inter-component communication on the EventBus.
- **Configuration**: System and service configuration data.
- **Secrets**: API keys, tokens, certificates, and passwords.
- **Service state**: In-memory and persisted state of running services.
- **Audit logs**: Records of security-relevant events.
- **User data**: Data processed by platform services.

### Trust Boundaries

```
[ External Network ]
        |
    [ API Gateway / Load Balancer ]
        |
    [ AI-OS Platform ]
        |--- [ Core Platform ]
        |       |--- EventBus (trusted internal bus)
        |       |--- Registry (service discovery)
        |--- [ Runtime Platform ]
        |       |--- Permission Checker
        |       |--- Resource Manager
        |--- [ System Platform ]
        |       |--- Policy Engine
        |       |--- Plugin System
        |--- [ Services ]
                |--- Internal services
                |--- External integrations
```

### Threat Agents

| Agent | Motivation | Capability |
|---|---|---|
| External attacker | Data exfiltration, service disruption | Network access, limited system knowledge |
| Malicious service | Escalate privileges, access unauthorized data | Internal EventBus access, limited sandbox |
| Compromised dependency | Supply chain attack | Code execution at dependency's privilege level |
| Insider | Data theft, sabotage | Authorized access, system knowledge |

---

## Key Security Principles

### Least Privilege

Every component operates with the minimum permissions necessary to perform its function:

- Services start with zero permissions. Permissions are granted explicitly via the Policy Engine.
- The Permission Checker evaluates every operation against granted permissions.
- No service may access resources outside its declared capabilities.

```rust
// Permission check enforced at the EventBus middleware layer
pub struct PermissionMiddleware {
    checker: Arc<PermissionChecker>,
}

impl Middleware for PermissionMiddleware {
    async fn handle(&self, event: &Event, next: Next<'_>) -> Result<(), BusError> {
        // Check that the source module has permission to dispatch this event type
        self.checker
            .check_permission(
                &event.security_context,
                &event.event_type,
            )
            .await
            .map_err(|_| BusError::PermissionDenied)?;

        next.handle(event).await
    }
}
```

### Defense in Depth

Security controls are implemented at multiple layers:

| Layer | Control |
|---|---|
| Network | Firewall, TLS termination, rate limiting |
| API | Authentication, request validation |
| EventBus | Permission middleware, event authorization |
| Service | Capability-based access, resource limits |
| OS | Mandatory Access Control (SELinux/AppArmor), cgroups |
| Audit | Full audit trail of security-relevant events |

### Fail Secure

When a security check fails:
1. The operation is denied by default.
2. A `security.permission.denied` event is dispatched on the EventBus.
3. The event is logged with full context (principal, resource, action, timestamp).
4. The denial is audited.

No operation proceeds when a security control is unavailable. If the Permission Checker is down, the system denies all operations rather than allowing them.

### Secure by Default

- Default-deny for all permissions.
- Default-deny for all event subscriptions (explicit grant required).
- Default TLS for all network communication.
- Default minimum for resource limits.
- No default secrets or passwords.

---

## Permission System

The permission system is role-based and enforced at the Runtime Platform layer.

### Architecture

```
Event dispatch request
    |
    v
EventBus middleware pipeline
    |
    v
PermissionMiddleware
    |
    v
PermissionChecker
    |--- Identifies principal from SecurityContext
    |--- Loads role assignments
    |--- Evaluates policy rules
    |--- Returns Allow / Deny
```

### Permission Structure

```rust
pub struct Permission {
    pub principal: Principal,   // User, service, or agent ID
    pub action: Action,         // Event type or resource action
    pub resource: Resource,     // Target resource or event namespace
}
```

### Role Definitions

Roles are defined in configuration and evaluated at runtime:

```toml
# configs/permissions.toml
[[roles]]
name = "core.service"
permissions = [
    "core.events.publish",
    "core.events.subscribe",
    "core.registry.register",
]

[[roles]]
name = "runtime.scheduler"
permissions = [
    "runtime.task.*",
    "runtime.resource.allocate",
]
```

---

## Secrets Management

### Principles

1. **No secrets in code**: Secrets must never be hardcoded, committed to version control, or embedded in binaries.
2. **No secrets in configuration files**: Configuration files may reference secrets by name but must not contain secret values.
3. **Encryption at rest**: All stored secrets are encrypted using authenticated encryption (AES-256-GCM).
4. **Encryption in transit**: All network communication uses TLS 1.3.
5. **Rotation**: Secrets have expiration dates and are rotated automatically.

### Secret Storage

Secrets are stored in one of the following, in order of preference:

1. **Hardware Security Module (HSM)** — For production deployments.
2. **HashiCorp Vault** — For production and staging.
3. **Encrypted environment variables** — For development only.

### Environment Variable Conventions

```
AI_OS_SECRET_<SERVICE>_<KEY>
```

Examples:
- `AI_OS_SECRET_DATABASE_PASSWORD`
- `AI_OS_SECRET_API_GITHUB_TOKEN`
- `AI_OS_SECRET_TLS_PRIVATE_KEY`

### Secret Scanning

Pre-commit hooks scan for secrets:

```bash
# Install and run detect-secrets
pip install detect-secrets
detect-secrets scan --baseline .secrets.baseline

# Or use git-secrets
git secrets --scan
```

CI also scans for accidentally committed secrets.

---

## Input Validation

### Event Payload Validation

All event payloads are validated at the EventBus middleware layer before delivery to handlers:

```rust
pub struct ValidationMiddleware;

impl Middleware for ValidationMiddleware {
    async fn handle(&self, event: &Event, next: Next<'_>) -> Result<(), BusError> {
        // Validate event structure
        if event.payload.is_empty() {
            return Err(BusError::InvalidPayload("empty payload"));
        }

        // Schema validation (when schemas are defined)
        if let Some(schema) = self.get_schema(&event.event_type) {
            schema.validate(&event.payload)?;
        }

        next.handle(event).await
    }
}
```

### Validation Rules

1. Reject malformed input at the boundary (EventBus middleware).
2. Validate types, ranges, lengths, and formats.
3. Use Serde's `#[serde(deny_unknown_fields)]` to reject unexpected fields.
4. Use bounded types rather than unbounded strings (`u32` instead of `String`).
5. Sanitize any data that will be logged or displayed.

### Injection Prevention

- All events are serialized/deserialized via Serde (no raw string interpolation).
- Shell commands are never constructed from event payloads.
- File paths are validated against allowed prefixes.
- SQL (when introduced in Phase 5) uses parameterized queries exclusively.

---

## Audit Logging

### What Is Audited

All security-relevant events are audited:

| Event Type | Audit Record |
|---|---|
| Permission granted/denied | Principal, resource, action, decision, timestamp |
| Authentication event | Principal, method, success/failure, timestamp |
| Service start/stop | Service name, action, principal, timestamp |
| Configuration change | Key, old value hash, new value hash, principal, timestamp |
| Secret access | Principal, secret name, timestamp (access, not value) |
| Privilege escalation | Principal, from_role, to_role, timestamp |

### Audit Log Format

Audit logs are structured JSON, written to a dedicated audit sink:

```json
{
  "timestamp": "2026-07-19T12:00:00Z",
  "sequence": 12345,
  "event_type": "security.permission.denied",
  "principal": {
    "id": "svc-scheduler",
    "type": "service"
  },
  "resource": "runtime.resource.critical",
  "action": "allocate",
  "decision": "deny",
  "reason": "insufficient_privileges",
  "trace_id": "abc-123-def-456"
}
```

### Audit Log Protection

- Audit logs are append-only. Existing entries cannot be modified.
- Audit logs are written to a separate partition or sink from application logs.
- Audit logs are encrypted at rest.
- Access to audit logs is itself audited.

---

## Vulnerability Reporting Process

### Responsible Disclosure

If you discover a security vulnerability in AI-native OS:

1. **Do not** open a public GitHub issue.
2. **Do not** disclose the vulnerability publicly.
3. **Do** email the security contact: `security@ai-os.local` (replace with actual contact).
4. **Do** include as much detail as possible: affected versions, steps to reproduce, impact assessment.

### Response Timeline

| Timeframe | Action |
|---|---|
| 24 hours | Acknowledgment of receipt |
| 72 hours | Initial assessment and severity classification |
| 7 days | Fix development (Critical/High severity) |
| 30 days | Fix development (Medium/Low severity) |
| Upon fix | Coordinated public disclosure |

### Severity Classification

| Severity | Definition | Response Time |
|---|---|---|
| Critical | Remote code execution, privilege escalation, data exfiltration | 7 days |
| High | Denial of service, unauthorized data access | 14 days |
| Medium | Limited information disclosure, configuration weaknesses | 30 days |
| Low | Minor information leaks, best practice violations | Next release |

---

## Dependency Scanning

### Automated Scanning

All dependencies are scanned for known vulnerabilities using `cargo audit`:

```bash
# Run audit
cargo audit

# Update vulnerability database
cargo audit bin
# (or manually: cargo audit fetch)

# Check for yanked crates
cargo audit --ignore-source=yanked
```

### CI Integration

`cargo audit` runs as part of the CI pipeline. A failing audit blocks the build.

### Dependency Pinning

- All dependencies are pinned to specific versions in `Cargo.lock`.
- The lockfile is committed to version control.
- Dependencies are updated intentionally, not automatically.
- Before updating a dependency, review its changelog for security-related changes.

### License Scanning

Before adding a new dependency, verify its license is compatible with the project (MIT):

```bash
cargo license
```

---

## Secure Coding Guidelines

### General

1. Follow the [coding standards](coding-standards.md) — many security issues are prevented by standard Rust safety guarantees.
2. Minimize `unsafe` code. The project bans `unsafe` except for reviewed FFI bindings.
3. Avoid platform-specific code paths that may have different security behaviors.
4. Do not log secrets, passwords, tokens, or private keys.

### Event Handling

1. Validate all event payloads at the EventBus boundary, not in individual handlers.
2. Do not dispatch events containing secrets on the EventBus.
3. Verify the sender's identity before processing events with security implications.
4. Handle malformed events by returning errors, not by silently ignoring them.

### Error Handling

1. Do not expose internal details in error messages returned to external callers.
2. Log full error details internally; return sanitized errors externally.
3. Use `CoreError` and `RuntimeError` typed errors — never leak `Result::Err` details that contain internals.

### Concurrency

1. Use `tokio::sync` primitives (not `std::sync`) inside async code.
2. Avoid holding locks across `.await` points. When unavoidable, document the locking order.
3. Use `Arc` for shared ownership, never raw pointers or `unsafe` sharing.

### Cryptography

1. Use the `ring` or `rustls` crate (not OpenSSL bindings directly).
2. Use AEAD ciphers (AES-256-GCM or ChaCha20-Poly1305) for encryption.
3. Use Argon2id for password hashing.
4. Use Ed25519 for signing.
5. Do not implement cryptographic primitives. Use well-audited libraries.
6. Do not roll your own cryptography.

---

## Quick Reference

| Task | Command / Action |
|---|---|
| Audit dependencies | `cargo audit` |
| Secret scan | `detect-secrets scan .` |
| License check | `cargo license` |
| Report vulnerability | Email `security@ai-os.local` |
| Check TLS config | Verify `configs/tls.toml` |
| Review permissions | Check `configs/permissions.toml` |

---

## See Also

- [Design Principles](principles.md#principle-4-security-by-default) — Security as a design principle.
- [Architecture](architecture.md) — Security enforcement at each layer.
- [Deployment Guide](deployment.md) — Production security configuration.
- [Coding Standards](coding-standards.md) — Unsafe code policy and secure coding.
