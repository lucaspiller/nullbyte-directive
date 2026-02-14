# Emulator Core Agent Guide

This crate is the core emulator implementation for Nullbyte Directive.

## Current structure

- `src/lib.rs`: crate root with docs and dependency-intent imports used to
  satisfy workspace lint policy while implementation is still scaffolded.
- `Cargo.toml`: crate metadata, feature flags (`serde` optional), strict lint
  policy, and baseline dependencies.

## Development workflow

Run commands from the repository root:

- `make fmt` to format Rust and Markdown files.
- `make fmt-check` to verify formatting in CI mode.
- `make clippy` to run lint checks across all targets with warnings denied.
- `make test` to run workspace tests.
- `make coverage` to generate LCOV coverage output (`target/llvm-cov/lcov.info`)
  when `cargo-llvm-cov` is installed.
- `make conformance` to run conformance-focused `emulator-core` tests.
- `make fuzz` to check fuzz harness availability (`cargo-fuzz` required).
- `make hardening` to run Phase 14 stress/determinism hardening tests.
- `make determinism-fingerprint` to print the CI cross-host determinism
  fingerprint.

You can also run crate-only checks:

- `cargo check -p emulator-core`
- `cargo clippy -p emulator-core --all-targets -- -D warnings`
- `cargo test -p emulator-core`

### ISA Conformance Tests

The ISA test suite validates instruction behavior using literate markdown
programs in `tests/isa/*.n1.md`. Each test file targets a specific instruction
class. Tests run automatically with `cargo test` via `tests/isa_conformance.rs`.

## Important constraints

- Keep naming consistent: refer to the project as "Nullbyte Directive".
- The crate lint policy denies `unused_crate_dependencies`; keep explicit
  crate-root imports for declared dependencies until they are used in real code
  paths.
- Network constraints may block fetching new dependencies if `index.crates.io`
  is unreachable in the environment.
