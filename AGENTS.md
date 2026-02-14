Run these commands from the repository root:

- `make fmt`: Format all Rust and Markdown source files.
- `make clippy`: Run lint checks across all workspace targets (warnings are
  denied).
- `make test`: Run all tests in the workspace.
- `make conformance`: Run specific conformance-tagged tests for `emulator-core`.
- `make hardening`: Run stress and determinism hardening suites.

## Component Guides

### Emulator Core (`crates/emulator-core`)

ISA implementation, memory models, deterministic execution. See
@crates/emulator-core/AGENTS.md

### WASM Bindings (`crates/emulator-wasm`)

Bridging Rust logic to the browser.

### Debug Tool (`apps/debug-tool`)

UI state, visualization, and user interaction. Stack: Svelte + Vite + Tailwind
CSS.
