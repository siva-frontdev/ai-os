# API Reference

This directory contains the API reference documentation for the AI-native Operating Platform.

## Contents

| Document | Module | Status |
|---|---|---|
| _To be generated_ | Core Platform (`ai_os_core`) | Planned |
| _To be generated_ | Runtime Platform (`ai_os_runtime`) | Planned |
| _To be generated_ | System Platform | Planned |
| _To be generated_ | Memory Platform | Planned |
| _To be generated_ | Brain Platform | Planned |
| _To be generated_ | Perception Platform | Planned |
| _To be generated_ | Execution Platform | Planned |

## Generation

API documentation is generated from Rustdoc comments using `cargo doc`. 

```bash
cargo doc --no-deps --open
```

The generated output appears in `target/doc/`. 

For published API references, run:

```bash
cargo doc --no-deps --document-private-items
```

Then copy the contents of `target/doc/` to a documentation server.

## Cross-References

- [Architecture Overview](../architecture/overview.md)
- [Core Module](../modules/core.md)
- [Runtime Module](../modules/runtime.md)
- [Coding Standards](../coding-standards.md)
