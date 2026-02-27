# Emulator Debug Session Transport

`emulator-debug-session` owns runtime state and exposes a transport-agnostic
Rust API (`DebugSessionHost`).

The concrete debugger-facing transport is now:

- Bin: `nullbyte-dap`
- Path: `crates/emulator-debug-session/src/bin/nullbyte-dap.rs`
- DAP transport: stdin/stdout framed with the `dap` crate (`dap::Server`)
- Snapshot transport: HTTP on `127.0.0.1:48765`

## DAP Surface

`nullbyte-dap` implements the Phase 27 baseline request set:

- `initialize`
- `launch`
- `setBreakpoints`
- `continue`
- `next`
- `pause`
- `stackTrace`
- `scopes`
- `variables`
- `disconnect`
- custom `nullbyte.reload`

## HTTP Snapshot API

The embedded HTTP server is read-only and exposes only:

- `GET /v1/health`
- `GET /v1/snapshot?before=<n>&after=<n>`

No execution control endpoints are provided by this server.

## Used By

- VS Code extension (`apps/vscode-extension/src/extension.js`) launches
  `cargo run -p emulator-debug-session --bin nullbyte-dap` as the debug adapter.
- debug-tool attach mode consumes `GET /v1/snapshot` from the same process.
