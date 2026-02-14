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

## Include Expansion (Pass 0)

The `include` module handles recursive `.include` directive expansion before the
main assembly pipeline. Key features:

- **Path resolution**: Include paths are resolved relative to the containing
  file's directory.
- **Format detection**: `.n1.md` files have `n1asm` code blocks extracted; `.n1`
  files are treated as raw assembly.
- **Circular detection**: Uses canonical paths to detect and report circular
  includes.
- **Include chains**: Each expanded line carries its origin file path, line
  number, and the full include chain for error reporting.

Use `expand_includes(path)` to recursively expand all includes and produce a
flat list of `ExpandedLine` items ready for parsing.
