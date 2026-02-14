# Nullbyte Directive

This repository hosts the Nullbyte Directive emulator workspace.

## Workspace Task Runner

Use the root `Makefile` to run standard development commands:

- `make fmt` - format Rust and Markdown sources.
- `make fmt-check` - verify formatting without changing files.
- `make clippy` - run Clippy for all workspace targets with warnings denied.
- `make test` - run all workspace tests.
- `make coverage` - generate LCOV output at `target/llvm-cov/lcov.info`
  (requires `cargo-llvm-cov`).
- `make fuzz` - run fuzzing entrypoint checks (requires `cargo-fuzz`).
- `make conformance` - run conformance-tagged emulator-core tests.
- `make hardening` - run Phase 14 hardening test suite.
- `make determinism-fingerprint` - print deterministic replay fingerprint used by CI cross-host checks.

### Prerequisites

- Rust toolchain from `rust-toolchain.toml`
- `yarn install` for Markdown formatting (`prettier`)
- Optional: `cargo install cargo-fuzz` for `make fuzz`
- Optional: `cargo install cargo-llvm-cov` for `make coverage`

## emulator-core Docs

- Crate docs and integration guidance: `crates/emulator-core/README.md`
- Snapshot migration/version policy: `docs/emulator/snapshot-versioning-policy.md`
- `v0.1.0` release notes: `docs/emulator/emulator-core-v0.1.0-release-notes.md`
