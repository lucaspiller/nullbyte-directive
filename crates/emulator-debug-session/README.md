# Emulator Debug Session Transport

`emulator-debug-session` owns runtime state and exposes a transport-agnostic
Rust API.

For tool integration, the repository provides a concrete stdio transport binary:

- Bin: `debug-session-stdio-host`
- Path: `crates/emulator-debug-session/src/bin/debug-session-stdio-host.rs`
- Framing: newline-delimited JSON (`\n` terminated)
- Topology: one request line in, one response line out

## Wire Format

Request envelope:

```json
{ "id": 1, "method": "read_state", "params": {} }
```

Response envelope (success):

```json
{
  "id": 1,
  "ok": true,
  "result": {
    "paused": true,
    "run_state": "HaltedForTick",
    "registers": { "PC": 4096 }
  }
}
```

Response envelope (error):

```json
{ "id": 1, "ok": false, "error": "invalid params: ..." }
```

Contract rules:

- `id` is caller-assigned and echoed back in the response.
- `method` is a string command name.
- `params` is optional JSON; omitted params are treated as `{}`.
- Unknown methods return `ok=false` with an error string.

## Methods

`launch`

- Params: `{ "program": "<absolute-or-workspace-relative-path>" }`
- Result:
  `{ "ids": {"debug_session_id","build_id","layout_id"}, "warnings": ["..."] }`

`set_breakpoints`

- Params: `{ "file": "<path>", "lines": [10, 12] }`
- Result: `{ "breakpoints": [{"line":10,"verified":true,"message":null}] }`

`reload`

- Params: `{ "program": "<path>", "mode": "hot_patch" | "clean_run" }`
- Result:
  `{ "ids": {...}, "applied_mode": "hot_patch|clean_run", "clean_run_reason": "...|null", "warnings": [...] }`

`continue_execution`

- Params: `{}`
- Result:
  `{ "stop_reason": "breakpoint|pause|step|exception", "fault": "..." | null }`

`instruction_step`

- Params: `{}`
- Result: same shape as `continue_execution`

`tick_step`

- Params: `{}`
- Result: same shape as `continue_execution`

`pause`

- Params: `{}`
- Result: `{}`

`read_state`

- Params: `{}`
- Result:
  `{ "paused": bool, "run_state": string, "registers": {"R0":0,...,"PC":0,...} }`

`read_memory_window`

- Params: `{ "start": 0, "len": 64 }`
- Result: `{ "start": 0, "bytes": [0,1,2,...] }`

`read_visualizer_snapshot`

- Params: `{ "before": 3, "after": 4 }`
- Result: host visualizer snapshot payload from
  `DebugSessionHost::read_visualizer_snapshot`

`disconnect`

- Params: `{}`
- Result: `{}`

## Used By

- VS Code adapter client: `apps/vscode-extension/src/dap/session_host_client.js`
- Debug-tool attach mode (via VS Code bridge):
  - Bridge server: `apps/vscode-extension/src/dap/session_bridge_server.js`
  - Browser client: `apps/debug-tool/src/lib/session/sharedSessionHost.js`
