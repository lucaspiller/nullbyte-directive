# Debug Tool Agent Guide

This application is the browser debugger/visualizer for Nullbyte Directive. It
now supports two explicit modes selected by route.

## Context

- **Location**: `apps/debug-tool/`
- **Stack**: Svelte + Vite + Tailwind CSS
- **Integration**:
  - Standalone (`/`): consumes `emulator-wasm` (`WasmCore`) directly in-browser.
  - Attach (`/attach`): polls `emulator-debug-session` HTTP bridge
    (`GET /v1/snapshot`, `GET /v1/health`) for read-only visualization.
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
- **Mode Split Is Required**:
  - `/` must always work without a VS Code session or network bridge.
  - `/attach` is visualization-only; do not add run/step/pause/reset or control
    arbitration flows to attach mode.
- **Attach API Surface**:
  - Keep attach mode read-only. Avoid adding `POST /v1/*` call sites in
    `apps/debug-tool`.
- **Terminal Aesthetic**: maintain the "hacker/terminal" look.
  - Use `font-mono` for all data displays.
  - Use high-contrast colors (green/amber/red on dark slate).
  - Avoid rounded corners or soft shadows unless necessary for clarity.
- **Performance**: The debug loop runs frequently (e.g., 60fps when running).
  - Use `Uint8Array` views for memory instead of copying large arrays.
  - Batch UI updates if possible.
- **State Management**:
  - Standalone mode source of truth is local `WasmCore`.
  - Attach mode source of truth is snapshot payloads from the shared session
    host bridge.
  - Svelte components should react to snapshot/state updates; avoid duplicating
    emulator policy in JS.

## Directory Structure

- `src/lib/components/`: Reusable UI components (RegisterView, MemoryView,
  etc.).
- `src/lib/session/sharedSessionHost.js`: read-only attach polling client.
- `src/wasm/`: Destination for the compiled WASM module (gitignored, built via
  `npm run build:wasm`).
- `src/App.svelte`: Route-aware shell (`/` standalone, `/attach` attach).
