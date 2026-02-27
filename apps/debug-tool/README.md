# Nullbyte Directive Debug Tool

The debug-tool supports two routes:

- `/` (standalone): runs `emulator-wasm` (`WasmCore`) directly in-browser with
  local run/step/pause/reset and file load.
- `/attach` (attach mode): read-only visualization of a live VS Code debug
  session via bridge snapshot polling.

## Run locally

From `apps/debug-tool`:

1. `npm install`
2. `npm run dev`

Open:

- Standalone mode: `http://localhost:5173/`
- Attach mode: `http://localhost:5173/attach`

## Standalone mode (`/`)

- No VS Code debug session required.
- No network bridge required.
- Uses in-browser `WasmCore`.
- Supports:
  - run/pause
  - step
  - reset
  - speed toggle
  - `.n1` / `.n1.md` source load

## Attach mode (`/attach`)

- Polls `GET /v1/snapshot` (plus `GET /v1/health`) from the DAP-hosted bridge.
- Read-only visualization only (no run/step/pause/reset controls).
- If no active session exists, `/attach` shows a clear "no active attach
  session" message.

## Bridge configuration

- Default bridge URL: `http://127.0.0.1:48765`
- Override with:
  - `VITE_SESSION_BRIDGE_URL=http://127.0.0.1:48765 npm run dev`

## Build

- `npm run build`

## Key files

- App shell: `apps/debug-tool/src/App.svelte`
- Attach polling client: `apps/debug-tool/src/lib/session/sharedSessionHost.js`
