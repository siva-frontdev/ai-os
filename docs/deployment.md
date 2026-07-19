# Deployment Guide

## Deployment Architecture

### Current Architecture (Single-Node)

The current deployment is a single-node setup running on AWS EC2 with Arch Linux. All AI-native OS components run on the same host, isolated by the operating system's process and user permissions.

```
Internet
    |
    v
AWS EC2 Instance (c6i.xlarge or larger)
    |
    +-- Arch Linux (LTS)
    |       |
    |       +-- AI-OS Platform
    |       |       +-- Core Platform (EventBus, Lifecycle, Registry)
    |       |       +-- Runtime Platform (Scheduler, Supervisor, Session)
    |       |       +-- System Platform (Daemon, Policy, Plugin)
    |       |       +-- Platform Services
    |       |
    |       +-- System Services
    |       |       +-- systemd (service management)
    |       |       +-- Prometheus Node Exporter (metrics)
    |       |       +-- rsyslog or journald (logging)
    |       |
    |       +-- Monitoring Stack
    |               +-- Prometheus (metrics collection)
    |               +-- Grafana (optional, dashboards)
    |
    +-- Security Group
            +-- Ingress: 22 (SSH), 9090 (health/metrics), 443 (API, future)
            +-- Egress: all required
```

### Future Architecture (Multi-Node)

As the platform grows, the architecture will evolve to a multi-node deployment:

```
Internet
    |
    v
Load Balancer (ALB / NLB)
    |       |       |
    v       v       v
Worker1  Worker2  Worker3   (AI-OS Platform Nodes)
    |       |       |
    +-------+-+------+
            |
            v
    Core Services (EventBus, Registry)
            |
            v
    Data Layer (Memory Platform, Storage)
```

Multi-node deployment is planned for Phase 5 and beyond.

---

## Prerequisites

### AWS EC2 Instance

- **AMI**: Arch Linux (current LTS)
- **Instance type**: `c6i.xlarge` (4 vCPU, 8 GB RAM) minimum
- **Storage**: 50 GB gp3 root volume minimum, 100 GB recommended
- **Security group**:
  - SSH (22) from trusted IPs only
  - Health check endpoint (9090) from monitoring CIDR
  - API endpoint (443) from load balancer CIDR
- **IAM role**: If using AWS services (optional), attach appropriate permissions

### System Dependencies

```bash
# Connect to the instance
ssh -i <key> arch@<instance-ip>

# Install AI-OS dependencies
sudo pacman -Syu
sudo pacman -S --needed \
    base-devel \
    git \
    rustup \
    openssl \
    pkg-config \
    prometheus-node-exporter \
    systemd-libs
```

---

## Systemd Service Files

### Core Platform Service

Create `/etc/systemd/system/ai-os-core.service`:

```ini
[Unit]
Description=AI-OS Core Platform
After=network.target
Wants=network-online.target

[Service]
Type=simple
User=ai-os
Group=ai-os
WorkingDirectory=/opt/ai-os
Environment=RUST_LOG=info
Environment=AI_OS_CONFIG_DIR=/etc/ai-os
Environment=AI_OS_DATA_DIR=/var/lib/ai-os
ExecStart=/opt/ai-os/bin/ai-os-core --config /etc/ai-os/config.toml
Restart=on-failure
RestartSec=5
StandardOutput=journal
StandardError=journal
NoNewPrivileges=yes
PrivateTmp=yes
ProtectSystem=strict
ProtectHome=yes
ReadWritePaths=/var/lib/ai-os /var/log/ai-os

[Install]
WantedBy=multi-user.target
```

### Runtime Platform Service

Create `/etc/systemd/system/ai-os-runtime.service`:

```ini
[Unit]
Description=AI-OS Runtime Platform
After=ai-os-core.service
Requires=ai-os-core.service

[Service]
Type=simple
User=ai-os
Group=ai-os
WorkingDirectory=/opt/ai-os
Environment=RUST_LOG=info
Environment=AI_OS_CONFIG_DIR=/etc/ai-os
ExecStart=/opt/ai-os/bin/ai-os-runtime --config /etc/ai-os/config.toml
Restart=on-failure
RestartSec=5
StandardOutput=journal
StandardError=journal
NoNewPrivileges=yes
PrivateTmp=yes
ProtectSystem=strict
ProtectHome=yes
ReadWritePaths=/var/lib/ai-os /var/log/ai-os

[Install]
WantedBy=multi-user.target
```

### Enabling, Starting, and Viewing Logs

```bash
sudo systemctl daemon-reload
sudo systemctl enable ai-os-core ai-os-runtime
sudo systemctl start ai-os-core ai-os-runtime
sudo journalctl -u ai-os-core -f
```

---

## Environment Configuration

### Directory Structure

```
/etc/ai-os/
  config.toml           -- Main platform configuration
  permissions.toml      -- Permission role definitions
  logging.toml          -- Logging configuration
  tls/                  -- TLS certificates (if applicable)
    cert.pem
    key.pem

/var/lib/ai-os/
  data/                 -- Persistent data storage
  state/                -- Runtime state files

/var/log/ai-os/
  platform.log          -- Main log output
  audit.log             -- Security audit log

/opt/ai-os/
  bin/
    ai-os-core          -- Core platform binary
    ai-os-runtime       -- Runtime platform binary
  configs/              -- Default configuration files
```

### Configuration File

```toml
# /etc/ai-os/config.toml
[platform]
name = "ai-os-production"
environment = "production"

[logging]
level = "info"
format = "json"
directory = "/var/log/ai-os"

[event_bus]
capacity = 10000
middleware = ["logging", "tracing", "permission"]

[health]
enabled = true
bind_address = "0.0.0.0:9090"

[security]
tls_enabled = true
tls_cert_path = "/etc/ai-os/tls/cert.pem"
tls_key_path = "/etc/ai-os/tls/key.pem"

[resources]
max_memory_mb = 4096
max_cpu_percent = 80
max_open_files = 65536
```

### Environment Variables

| Variable | Default | Description |
|---|---|---|
| `RUST_LOG` | `info` | Log level filter |
| `AI_OS_CONFIG_DIR` | `/etc/ai-os` | Configuration directory |
| `AI_OS_DATA_DIR` | `/var/lib/ai-os` | Data directory |
| `AI_OS_LOG_DIR` | `/var/log/ai-os` | Log directory |
| `AI_OS_HEALTH_BIND` | `0.0.0.0:9090` | Health check bind address |
| `AI_OS_NODE_NAME` | hostname | Node identifier in logs/metrics |

---

## Health Check Endpoints

The platform exposes health check endpoints on port `9090` (configurable via `AI_OS_HEALTH_BIND`):

### Liveness Endpoint

```
GET /health/live
```

Returns `200 OK` if the platform process is alive and running. Used by container orchestrators and load balancers to detect process death.

```json
{
  "status": "ok",
  "timestamp": "2026-07-19T12:00:00Z",
  "uptime_seconds": 86400
}
```

### Readiness Endpoint

```
GET /health/ready
```

Returns `200 OK` if the platform is ready to accept work. Returns `503 Service Unavailable` during startup or graceful shutdown.

```json
{
  "status": "ready",
  "timestamp": "2026-07-19T12:00:00Z",
  "services": {
    "core": "running",
    "runtime": "running",
    "scheduler": "running",
    "supervisor": "running"
  }
}
```

### Health Metrics

```
GET /health/metrics
```

Returns Prometheus-format metrics for scraping:

```
# HELP ai_os_up Is the AI-OS platform running
# TYPE ai_os_up gauge
ai_os_up 1
# HELP ai_os_services_total Total number of registered services
# TYPE ai_os_services_total gauge
ai_os_services_total 12
# HELP ai_os_events_total Total events dispatched
# TYPE ai_os_events_total counter
ai_os_events_total 48321
# HELP ai_os_events_per_second Event dispatch rate
# TYPE ai_os_events_per_second gauge
ai_os_events_per_second 1250.5
```

---

## Monitoring Setup

### Prometheus Stack

```bash
# Node Exporter (system-level metrics)
sudo pacman -S prometheus-node-exporter
sudo systemctl enable --now prometheus-node-exporter

# Prometheus server (optional, standalone)
sudo pacman -S prometheus
cat > /tmp/prometheus.yml << 'EOF'
global:
  scrape_interval: 30s
scrape_configs:
  - job_name: 'ai-os'
    static_configs:
      - targets: ['localhost:9090']
  - job_name: 'node'
    static_configs:
      - targets: ['localhost:9100']
EOF
sudo mv /tmp/prometheus.yml /etc/prometheus/prometheus.yml
sudo systemctl enable --now prometheus

# Grafana (optional dashboards)
sudo pacman -S grafana
sudo systemctl enable --now grafana
# Access at http://<instance-ip>:3000 (default admin/admin)
```

### Key Metrics

| Metric | Source | Threshold | Action |
|---|---|---|---|
| CPU usage | Node Exporter | > 80% for 5 min | Add capacity |
| Memory usage | Node Exporter | > 80% | Add capacity or reduce load |
| Events per second | AI-OS /metrics | > 80% of capacity | Scale event bus |
| Event queue depth | AI-OS /metrics | > 50% of capacity | Add worker threads |
| Service health | AI-OS /health/ready | < 100% services up | Investigate failed services |
| Error rate | AI-OS /metrics | > 1% of events | Investigate errors |

---

## Log Aggregation

All service logs are sent to systemd-journald. View them with:

```bash
sudo journalctl -u ai-os-core -f
sudo journalctl -u ai-os-core -o json  # JSON format for parsing
```

### Log Rotation

Configure log rotation in `/etc/systemd/journald.conf`:

```ini
[Journal]
SystemMaxUse=1G
MaxFileSec=1month
ForwardToSyslog=no
```

### Remote Log Aggregation

Forward logs to a centralized platform with Vector or Fluentd:

```bash
sudo pacman -S vector
cat > /etc/vector/vector.toml << 'EOF'
[sources.journald]
type = "journald"
units = ["ai-os-core", "ai-os-runtime"]
[sinks.elasticsearch]
type = "elasticsearch"
inputs = ["journald"]
endpoint = "https://logs.example.com:9200"
index = "ai-os-logs-%Y-%m-%d"
EOF
sudo systemctl enable --now vector
```

---

## Backup Strategy

### What to Back Up

| Item | Frequency | Retention | Storage |
|---|---|---|---|
| Configuration files (`/etc/ai-os/`) | On change | Latest + 5 versions | Git + S3 |
| Persistent data (`/var/lib/ai-os/data/`) | Daily | 30 days | S3 |
| Audit logs (`/var/log/ai-os/audit.log`) | Daily | 90 days | S3 / Glacier |
| Application logs (`/var/log/ai-os/`) | Daily | 14 days | S3 |
| Systemd journal | Daily | 30 days | S3 |

### Backup Commands

```bash
git -C /etc/ai-os add -A && git -C /etc/ai-os commit -m "config backup $(date +%Y-%m-%d)"
tar czf /tmp/ai-os-data-$(date +%Y-%m-%d).tar.gz /var/lib/ai-os/data/
aws s3 cp /tmp/ai-os-data-*.tar.gz s3://ai-os-backups/data/
```

Backups are verified monthly by restoration to staging. Failed backups generate an alert.

---

## Rollback Procedure

### Rollback Steps

1. **Stop services**: `sudo systemctl stop ai-os-core ai-os-runtime`.
2. **Restore binaries**: Copy the previous release artifacts.
3. **Restore configuration**: `sudo git -C /etc/ai-os checkout <previous-commit>`.
4. **Restore data**: `sudo tar xzf /var/backups/ai-os-data-<date>.tar.gz -C /`.
5. **Start services**: `sudo systemctl start ai-os-core ai-os-runtime`.
6. **Verify**: Check health endpoints and metrics.
7. **Document**: Record the rollback reason.

Rollback procedures are tested quarterly and after every major release.

---

## Scaling Considerations

### Vertical Scaling (Current)

- **CPU**: Upgrade instance type (`c6i.xlarge` -> `c6i.4xlarge`).
- **Memory**: Upgrade instance type.
- **Disk**: Increase EBS volume.
- **EventBus**: Increase channel capacity in configuration.

### Horizontal Scaling (Future, Phase 5+)

1. **Stateless services**: Add instances behind a load balancer.
2. **Stateful services**: Require distributed coordination (etcd).
3. **EventBus**: Requires partitioning or a distributed broker.
4. **Data layer**: Requires distributed storage.

### Load Testing

```bash
sudo pacman -S hey
hey -n 10000 -c 100 http://localhost:9090/health/ready
```

Document results for capacity planning.

---

## Deployment Checklist

Before deploying a new release to production:

- [ ] Release built and tested in CI.
- [ ] Binaries deployed to `/opt/ai-os/bin/`.
- [ ] Configuration files updated and reviewed.
- [ ] Previous binaries backed up (`ai-os-core.bak`, `ai-os-runtime.bak`).
- [ ] Database/data migration scripts ready (if applicable).
- [ ] Health check endpoints verified after deploy.
- [ ] Monitoring alerts configured and tested.
- [ ] Rollback binaries and configuration ready.
- [ ] Team notified of deployment window.

---

## Quick Reference

| Task | Command |
|---|---|---|
| Start all services | `sudo systemctl start ai-os-core ai-os-runtime` |
| View logs | `sudo journalctl -u ai-os-core -f` |
| Health check | `curl http://localhost:9090/health/live` |
| Metrics | `curl http://localhost:9090/health/metrics` |
| Backup data | `tar czf backup.tar.gz /var/lib/ai-os/data/` |

---

## See Also

- [Build Guide](build.md) — Building release binaries.
- [Release Process](release.md) — Versioning and release workflow.
- [Security Guide](security.md) — Production security configuration.
- [Architecture](architecture.md) — System architecture overview.
