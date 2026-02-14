# Debug Tool Agent Guide

This application is the browser-based debugger for Nullbyte Directive programs.
It provides an interactive interface to inspect the emulator state via
WebAssembly.

## Context

- **Location**: `apps/debug-tool/`
- **Stack**: Svelte + Vite + Tailwind CSS
- **Integration**: Consumes `emulator-wasm` (which wraps `emulator-core`)
- **Theme**: Dark, terminal-inspired, monospace-first

## Development Workflow

Run commands from `apps/debug-tool/`:

- `npm install`: Install frontend dependencies.
- `npm run dev`: Start the local development server.
- `npm run build`: Build the production bundle.
- `npm run build:wasm`: Compile the Rust `emulator-wasm` crate into a WASM
  module usable by the frontend. **Must be run before `dev` or `build`**.

## Key Constraints & Conventions

- **WASM First**: The emulator core logic resides in Rust. The frontend should
  only handle UI state and rendering. Do not reimplement core emulation logic in
  JS.
- **Terminal Aesthetic**: maintain the "hacker/terminal" look.
  - Use `font-mono` for all data displays.
  - Use high-contrast colors (green/amber/red on dark slate).
  - Avoid rounded corners or soft shadows unless necessary for clarity.
- **Performance**: The debug loop runs frequently (e.g., 60fps when running).
  - Use `Uint8Array` views for memory instead of copying large arrays.
  - Batch UI updates if possible.
- **State Management**:
  - The `WasmCore` instance is the source of truth.
  - Svelte components should react to `state` updates triggered by `step()` or
    `run()`.

## Directory Structure

- `src/lib/components/`: Reusable UI components (RegisterView, MemoryView,
  etc.).
- `src/wasm/`: Destination for the compiled WASM module (gitignored, built via
  `npm run build:wasm`).
- `src/App.svelte`: Main application shell and state controller.
