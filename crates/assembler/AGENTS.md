# Assembler Agent Notes

## Scope

`crates/assembler` contains the Nullbyte Directive assembler crate and the
`nullbyte-asm` CLI binary.

## Build and Verification

Run from repository root:

- `cargo check -p assembler`
- `cargo clippy -p assembler --all-targets -- -D warnings`
- `cargo test -p assembler`

Workspace-level checks that this repository expects:

- `make fmt`
- `make clippy`
- `make test`
- `make conformance`
- `make hardening`

## CLI

Current scaffold supports argument parsing for:

- required input path (`<input>`)
- optional output path (`-o <output>`)
- verbose flag (`--verbose`)
- help (`--help`)

Assembly pipeline wiring is implemented in later phases.
