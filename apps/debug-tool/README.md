# Nullbyte Directive Debug Tool

The debug tool is a browser visualizer for a shared `emulator-debug-session`
runtime. In attach mode, it does not own emulator state; it reads and controls
the same session used by the VS Code debug adapter.

## Start the shared session path

1. Start VS Code debugging with the Nullbyte Directive extension:
   - Open this repository in VS Code
     (`/Users/luca/code/personal/nullbyte-directive`).
   - Open the Run and Debug view.
   - Use the workspace launch config in `.vscode/launch.json`:
     `Launch Nullbyte Directive`.
   - Optionally edit `"program"` to a different assembly source file.
   - Keep `"hotReloadOnSave": true` in the launch config to enable VS Code
     save-triggered hot reload.
   - Press **F5** (or click **Start Debugging**).
   - Wait for the first stop event (`reason: "entry"`). This confirms the DAP
     session and host runtime are live.
2. The DAP adapter launches `emulator-debug-session` via stdio and starts an
   HTTP bridge (`session_bridge_server.js`) on `127.0.0.1:48765` by default.
3. Start the debug tool from `apps/debug-tool`:
   - `npm install`
   - `npm run dev`

## Bridge configuration

- Default bridge URL: `http://127.0.0.1:48765`
- Override in debug-tool with Vite env var:
  - `VITE_SESSION_BRIDGE_URL=http://127.0.0.1:48765 npm run dev`

## Using attach mode

- The tool continuously polls `/v1/snapshot` for registers, memory, disassembly,
  and TELE-7 state.
- Use **Attach Ctrl** to request control ownership.
- Run/step/tick/reset actions are allowed only for the current control owner.
- Release control to hand execution back to VS Code or another client.

## Program loading in debug-tool

When the debug-tool owns control, it can submit source through
`/v1/load_source`. Supported files are `.n1` and `.n1.md`.

## Build

- `npm run build`

## Key files

- App shell: `apps/debug-tool/src/App.svelte`
- Shared attach client: `apps/debug-tool/src/lib/session/sharedSessionHost.js`
- VS Code bridge: `apps/vscode-extension/src/dap/session_bridge_server.js`
