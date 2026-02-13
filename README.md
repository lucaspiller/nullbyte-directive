# Nullbyte Directive

This repository hosts the Nullbyte Directive emulator workspace.

## Workspace Task Runner

Use the root `Makefile` to run standard development commands:

- `make fmt` - format Rust and Markdown sources.
- `make fmt-check` - verify formatting without changing files.
- `make clippy` - run Clippy for all workspace targets with warnings denied.
- `make test` - run all workspace tests.
- `make fuzz` - run fuzzing entrypoint checks (requires `cargo-fuzz`).
- `make conformance` - run conformance-tagged emulator-core tests.

### Prerequisites

- Rust toolchain from `rust-toolchain.toml`
- `yarn install` for Markdown formatting (`prettier`)
- Optional: `cargo install cargo-fuzz` for `make fuzz`
