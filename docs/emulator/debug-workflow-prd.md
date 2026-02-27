# Debug Workflow PRD

## Document Status

- Owner: Tooling
- Last Updated: 2026-02-27
- Status: In Progress v0.4 (Rust-native DAP adapter + simplified control model)
- Supersedes: v0.3 architecture (Node.js adapter retired)

## Delivery Snapshot (2026-02-27)

Implemented and retained from v0.3:

- `crates/emulator-debug-session` is the authoritative runtime owner with
  launch, pause/continue/step, source-first breakpoints, build/layout/session
  IDs, and hot-reload policy (`CleanRun`/`HotPatch`) plus deterministic fallback
  reasons. This crate is stable and is not changed in v0.4.
- Host conformance and hardening tests are wired into root `make conformance`
  and `make hardening`.

Targeted for retirement in v0.4:

- `debug-session-stdio-host` binary (JSON-line RPC transport replaced by DAP
  binary directly).
- `apps/vscode-extension/src/dap/adapter.js` (Node DAP adapter).
- `apps/vscode-extension/src/dap/session_host_client.js` (Node stdio RPC
  client).
- `apps/vscode-extension/src/dap/session_bridge_server.js` (Node HTTP bridge;
  replaced by Rust HTTP server inside the DAP binary).
- Control arbitration protocol (`POST /v1/control/request`,
  `POST /v1/control/release`): VS Code is now the unconditional control plane.
- Debug-tool control arbitration toggle ("ATTACH/DETACH CTRL").
- All execution control endpoints on the HTTP bridge (`POST /v1/step`,
  `/v1/tick`, `/v1/pause`, `/v1/reset`, `/v1/load_source`).

Known functional gaps driving v0.4 work:

- Breakpoints are stored and resolved but never checked during
  `continue_execution`; hitting a breakpoint does not pause execution.
- DAP `stackTrace` returns `line: 1, column: 1` unconditionally; VS Code cannot
  highlight the current source line.
- The `continue` handler blocks the entire DAP stdin/stdout pipe until a run
  boundary; `pause` cannot interrupt a running program.
- `debug-tool` standalone mode (no VS Code session) is broken; the tool has no
  fallback to `WasmCore` when the bridge is unavailable.

## Context

The original debug loop pain was editor context-switch friction: edit source,
switch to debug-tool, manually reload, recreate execution position. The v0.3
architecture addressed this structurally by introducing `emulator-debug-session`
as a shared host crate and wiring VS Code as the control plane through a DAP
adapter. The session host layer is sound; the problems to solve now are:

1. The v0.3 Node.js DAP adapter introduces a process boundary and language
   context that puts logic outside `cargo test` reach and slows iteration.
2. Breakpoints, source-line mapping in the editor, and non-blocking continue are
   not yet functional, which means the VS Code debug UX does not deliver its
   core value.
3. `debug-tool` standalone mode was inadvertently broken by the shift to
   bridge-only operation. There is no recovery path when no VS Code session is
   running.
4. The control arbitration model (two clients negotiate step ownership) adds
   protocol complexity without a real use case. The tools have distinct roles:
   VS Code controls, debug-tool visualizes.

## Problem Statement

We need a debugging architecture that:

- Lets engineers debug/step from VS Code with standard controls.
- Preserves execution state on safe code edits.
- Keeps `debug-tool` focused on visualization and peripherals.
- Maintains deterministic emulator behavior and conformance.

## Product Goals

1. Make edit-save-debug a single-surface loop in VS Code.
2. Provide deterministic hot reload with explicit safety rules.
3. Keep one authoritative emulator session per run.
4. Let `debug-tool` attach as a visualization client for machine/peripheral
   state.
5. Preserve compatibility with existing emulator-core and conformance suites.

## Non-Goals

1. Time-travel debugging in this phase.
2. Multi-user collaborative debugging.
3. Remote/cloud debug hosting.
4. Full symbolic stack unwinding beyond available metadata.

## Option Set

### Option A: Debug-Tool Centric Hot Reload

Keep browser tool as the control plane. Add stronger hot reload in the UI.

Pros:

- Fastest to ship.
- Minimal new components.

Cons:

- Does not solve editor context switching.
- Keeps stepping outside VS Code.
- Harder long-term fit for peripheral-heavy workflows.

### Option B: VS Code DAP + Dedicated Emulator Session Host

Make VS Code the control plane through DAP. Add a host process that owns the
emulator session. `debug-tool` is optional and read-only for visualization.

Pros:

- Best developer ergonomics for stepping/breakpoints.
- Clean separation: control in editor, visualization in debug-tool.
- Scales to richer peripherals without bloating editor UI.

Cons:

- Higher upfront scope: extension + adapter + host protocol.

### Option C: Shared Session Host First, DAP Second

Build shared host and protocol first, then add DAP once protocol stabilizes.

Pros:

- Lower integration risk.
- Reusable foundation for VS Code and debug-tool.

Cons:

- Delays native VS Code debug UX by one milestone.

## Recommendation

Choose **Option B**, implemented in phases so delivery risk stays controlled.

Reasoning:

- The user problem is editor-loop friction, not just reload correctness.
- Option B directly solves that while preserving debug-tool value for
  peripherals.
- The host boundary prevents UI-specific policy noise from leaking into core
  debugging semantics.

## Proposed Architecture

1. **`nullbyte-dap` binary** (new bin target in `crates/emulator-debug-session`)
   - Speaks DAP on stdin/stdout using the `dap` crate (`dap::Server` poll loop).
   - Owns `DebugSessionHost` directly — no intermediate JSON-RPC transport.
   - Concurrently serves a read-only HTTP API on `127.0.0.1:48765`
     (`GET /v1/snapshot`, `GET /v1/health`) for the debug-tool visualizer.
   - Launched by the VS Code extension via `cargo run -p emulator-debug-session
     --bin nullbyte-dap`.

2. **`apps/vscode-extension` (reduced)**
   - `extension.js` only: registers `DebugAdapterExecutable` pointing at the
     Rust binary, auto-fills launch config, and sends `nullbyte.reload` custom
     DAP request on `.n1`/`.n1.md` save.
   - All protocol logic removed; testable entirely through `cargo test`.

3. **`apps/debug-tool` (split routes)**
   - `/` — standalone mode: `WasmCore` in-browser, full execution controls, no
     network dependency.
   - `/attach` — attach mode: polls `GET /v1/snapshot`, read-only
     visualization, no execution controls rendered.

## Functional Requirements

### FR-1 Session Ownership

A single host process is the source of truth for run state, memory, and
peripherals.

### FR-2 Reload Modes

Host supports:

- `CleanRun`: reset core and reload full binary.
- `HotPatch`: patch ROM bytes only and preserve architectural/runtime state.

### FR-3 Reload Safety Contract

Hot patch is allowed only when:

- Assembly succeeds with no errors.
- Layout compatibility check passes.
- Core is paused at an instruction boundary.

Otherwise host falls back to `CleanRun` with explicit reason.

### FR-4 Build and Layout IDs

Host computes:

- `build_id`: content hash of emitted binary.
- `layout_id`: hash of address mapping shape (address + length + source
  location), not raw bytes.

If `build_id` is unchanged, reload is a no-op.

### FR-5 Source Mapping Quality

Assembler metadata for debugger flow must include real file/line information.
Placeholder line numbers are not acceptable for DAP breakpoint stability.

### FR-6 Breakpoint Semantics

Breakpoints are stored source-first and resolved through current layout mapping.
On layout change, breakpoints are remapped or marked unresolved with a clear
reason.

### FR-7 DAP Baseline

Initial DAP scope:

- `launch`
- `setBreakpoints`
- `continue`
- `next` (instruction step)
- `pause`
- `stackTrace` (minimal valid frame set)
- `scopes` + `variables` for registers and selected memory windows

### FR-8 Visualizer Subscription

In attach mode, `debug-tool` polls session snapshots from the HTTP bridge and
never issues execution control commands. VS Code is the unconditional control
plane; there is no negotiated arbitration.

### FR-9 Diagnostics

Assembly and runtime faults are surfaced in both VS Code and debug-tool using a
shared diagnostic model.

### FR-10 Determinism

Reload behavior must preserve deterministic semantics relative to explicit mode:

- `CleanRun`: deterministic from reset state.
- `HotPatch`: deterministic from pre-patch state plus patch set.

### FR-11 Include-Aware Assembly

Host assembly path uses file-based assembler entrypoints for launch/save-driven
flows so `.include` graphs are supported.

## Non-Functional Requirements

1. Host state update latency target: under 50 ms for step responses in typical
   local workflows.
2. Save-to-ready reload target: under 200 ms for small/medium programs.
3. No divergence from `emulator-core` conformance behavior.

## Milestones

1. **M1: Session Host + Protocol** (Completed)
   - Host crate and protocol implemented.
   - Build/layout IDs and reload policy implemented.
   - Reload safety and breakpoint remap tests in place.

2. **M2: VS Code DAP Integration** (Completed — Node.js adapter)
   - Extension + Node.js DAP adapter wired to stdio host transport.
   - Launch, stepping, breakpoints, register/memory scopes active.
   - Save-triggered hot reload path active for `.n1` / `.n1.md`.

3. **M3: Debug-Tool Visualizer Mode** (Completed — bridge-attach)
   - Attach-to-session UX via HTTP bridge active.
   - Peripheral panes consume host snapshot stream.

4. **M4: Hardening** (Completed for current scope)
   - Determinism checks for repeated patch cycles in host tests.
   - Conformance + hardening coverage included in root make targets.

5. **M5: Rust-Native DAP Adapter** (In Progress — Phase 27)
   - `nullbyte-dap` Rust binary replaces Node.js adapter.
   - Breakpoints fire during `continue`; `stackTrace` resolves real source
     lines; `continue` is non-blocking.
   - Extension reduced to ~50-line `extension.js`.

6. **M6: Debug-Tool Standalone/Attach Split** (Planned — Phase 28)
   - `/` standalone mode restored with `WasmCore`.
   - `/attach` is read-only visualization; no control affordances.
   - Control arbitration removed.

## Risks and Mitigations

1. **Risk:** Hot patch corrupts running semantics.
   - **Mitigation:** ROM-only patching, pause-required updates, strict fallback
     to clean run.

2. **Risk:** Breakpoint drift after layout changes.
   - **Mitigation:** Source-first breakpoint storage + explicit remap status.

3. **Risk:** DAP scope creep.
   - **Mitigation:** Ship minimal DAP baseline first, expand only after stable
     core workflows.

## Judgment Calls Made

1. **Control plane location:** VS Code is primary control plane.
   - Chosen because it directly removes context-switch friction.

2. **Hot patch boundary:** ROM-only in v1.
   - Chosen to avoid overwriting live RAM/peripheral state.

3. **Fallback behavior:** automatic clean run on unsafe patch.
   - Chosen for correctness-first behavior and predictable user outcomes.

4. **Branch salvage policy:** keep concepts, not UI implementation.
   - Chosen because previous UI changes mixed policy with presentation and added
     noise without strong correctness guarantees.

5. **DAP adapter language:** Rust binary using the `dap` crate, not Node.js.
   - Chosen so all debug logic is reachable by `cargo test` without a VS Code
     instance; eliminates the Node.js process boundary and its IPC overhead.

6. **VS Code as sole control plane:** no control arbitration protocol.
   - Chosen because negotiated ownership adds protocol complexity with no real
     use case. Debug-tool visualizes; VS Code controls. This is unconditional.

7. **Debug-tool route split (`/` standalone, `/attach` read-only):**
   - Chosen to restore standalone usability without a VS Code session, and to
     make the attach mode contract explicit: no execution controls are ever
     rendered in attach mode.

## Acceptance Criteria

1. Developer can set breakpoints and step from VS Code without using browser
   controls.
2. Saving compatible code changes resumes with preserved state in under target
   latency.
3. Incompatible changes trigger deterministic clean run with clear user-facing
   reason.
4. `debug-tool` can visualize the same live session, including peripherals,
   without owning execution control.
5. Conformance and hardening tests pass under host-mediated debug flow.
