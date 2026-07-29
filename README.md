# AI-OS Platform

An AI-native Operating Platform built on Arch Linux.

## Status: Living Beta

AI-OS is now in the Living Beta phase — evolution happens through real-world usage, not architectural redesign.

**Start here:** See the [Quick Start Guide](docs/quickstart.md).
**Daily use:** See the [Beta Guide](docs/beta-guide.md).
**Developer tools:** See the [Developer Mode](docs/developer-mode.md).
**Validation:** See the [Validation Checklist](docs/validation-checklist.md).
**Reporting issues:** Use the [Behavior Issue Template](docs/behavior-issue-template.md).

## Project Structure

```
ai-os/
├── core/          - Core system components
├── brain/         - AI orchestration and reasoning
├── memory/        - Memory and context management
├── runtime/       - Code execution runtime
├── perception/    - Input processing and sensors
├── execution/     - Action execution engine
├── services/      - Platform services
├── system/        - System integration
├── docs/          - Documentation
├── scripts/       - Build and utility scripts
├── tools/         - Development tools
├── tests/         - Test suites
├── configs/       - Configuration files
└── assets/        - Static assets
```

## Prerequisites

- Arch Linux
- Packages listed in `scripts/setup.sh`

## Quick Start

```bash
./scripts/setup.sh
```
