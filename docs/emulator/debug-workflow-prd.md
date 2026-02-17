# Debug Workflow PRD

## Document Status

- Owner: Tooling
- Last Updated: 2026-02-17
- Status: Draft v0.1
- Supersedes: `docs/emulator/debug-tool-prd.md` for debugging workflow scope

## Context

The current debug loop is browser-first:

1. Edit source in an editor.
2. Switch to `apps/debug-tool`.
3. Manually reload program input.
4. Recreate execution position after reset.

This creates two problems:

- High friction from repeated context switching.
- Lost state after each reload, which slows iteration.

The product direction also includes richer peripherals. That makes it more
important to separate "debug control" from "visualization," so each surface can
focus on its strengths.

## Investigation Summary

### Current `main` Baseline

`crates/emulator-wasm/src/lib.rs` already exposes useful primitives:

- `assemble_only`
- `patch_memory`
- `get_execution_metadata`
- `reset_and_reload`

These are enough to support hot-reload policy in a host layer.

### `debug-1-discarded` Assessment

Good ideas worth keeping:

- Explicit distinction between clean run and hot patch.
- Build/layout/session metadata as first-class concepts.

Changes to avoid repeating:

- Reload policy encoded directly in `apps/debug-tool/src/App.svelte`.
- Hot patch logic that effectively rewrites full binary and can clobber state.
- Incomplete metadata plumbing (`build_id`/`layout_id` not persisted in core
  session state).
- Source map fields with placeholder line data (`line = 0`), which is not
  sufficient for stable breakpoints.

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

1. **Debug Session Host** (new crate, suggested:
   `crates/emulator-debug-session`)
   - Owns authoritative `emulator-core` instance.
   - Runs assembly, stepping, breakpoints, reload policy.
   - Exposes protocol for DAP adapter and visualizer clients.

2. **VS Code Adapter**
   - Implements DAP requests/events.
   - Delegates execution control to Debug Session Host.
   - Renders diagnostics and source-level breakpoints in editor.

3. **Debug Tool Visualizer Mode**
   - Attaches to host session.
   - Displays memory/register/peripheral panels.
   - Does not own stepping policy by default.

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

`debug-tool` can subscribe to session snapshots/deltas without becoming the
control authority.

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

1. **M1: Session Host + Protocol**
   - New host crate with run/step/reload/breakpoint APIs.
   - Build/layout ID implementation and reload policy.
   - Unit tests for reload safety and breakpoint remap behavior.

2. **M2: VS Code DAP Integration**
   - Extension + DAP adapter wired to host.
   - Launch config, stepping, breakpoints, register/memory scopes.

3. **M3: Debug-Tool Visualizer Mode**
   - Attach-to-session UX.
   - Peripheral panes consume host stream.
   - Optional control affordances behind "secondary control" mode.

4. **M4: Hardening**
   - Cross-check determinism under repeated hot patch cycles.
   - Conformance + hardening suite coverage for host-mediated runs.

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
