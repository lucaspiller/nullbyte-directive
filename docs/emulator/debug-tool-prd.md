# Debug Tool PRD

## Document Status

- Owner: Tooling
- Last Updated: 2026-02-13
- Status: Draft v0.1 (Intro)

## Context

`debug-tool` is a browser-based, client-side debugger for Nullbyte One programs.
It runs the emulator through a WebAssembly adapter and gives engineers an
interactive way to inspect execution step by step.

This tool complements `emulator-core` by providing a consistent, human-usable
debug surface for validation, troubleshooting, and program authoring workflows.

## Problem Statement

We need a single debugger UI that:

- Runs the same emulator logic used by runtime integrations (via WASM build).
- Lets users step, pause, continue, and inspect machine state deterministically.
- Presents memory/register/event/fault information in a fast, inspectable
  terminal-style interface.

Without a dedicated debug tool, development and validation remain slower and
more error-prone due to fragmented scripts and low-level inspection workflows.

## Product Goals

1. Provide a reliable browser debugger powered by the WASM emulator.
2. Make execution state easy to inspect at instruction granularity.
3. Preserve deterministic behavior between debug runs and replay sessions.
4. Ship as a standalone app in `apps/debug-tool/`.
5. Use a clear Terminal UI style for dense technical workflows.

## Non-Goals (For This Intro PRD)

1. Full time-travel debugging and reversible execution.
2. Multiplayer/session collaboration features.
3. Remote server-hosted debugging.
4. Advanced profiling and performance analytics.

## Target Users

1. Emulator/core engineers validating ISA and fault semantics.
2. Tooling engineers building ROM and fixture workflows.
3. Content/systems engineers debugging Nullbyte programs.

## Proposed Product Scope (v0.1)

### In Scope

- Browser-only app shell in `apps/debug-tool/`.
- Stack: Vite + Svelte + Tailwind CSS.
- WASM emulator loading and lifecycle management.
- Program loading (initially local file/fixture based).
- Core controls: run, pause, step instruction, step tick, reset.
- Inspector panels for:
  - Registers/flags
  - Disassembly (current `PC` and nearby instructions)
  - Memory view
  - Event/fault output
  - Execution log/trace stream
- Terminal-inspired layout/theme optimized for high-density technical reading.

### Out of Scope

- Source-level language debugging.
- Cloud save/sync.
- Device-rich MMIO simulation beyond core adapter capabilities.

## UX Direction

The UI should feel like a modern terminal debugger:

- Monospace-first presentation and dense data tables.
- Keyboard-first navigation for common actions.
- Stable panel layout with low visual noise.
- Deterministic state updates tied to emulator step boundaries.

It will later be enhanced to allow the user to interact with peripherals
(storage devices, monitor, keyboard). That can be added as a seperate screen,
but should presnet a smaller version of the debugger.

## Technical Direction

- App location: `apps/debug-tool/`
- Frontend: Svelte on Vite
- Styling: Tailwind with a terminal-oriented design token layer
- Emulator integration: load WASM bundle, expose typed bridge for control and
  state snapshots
- State model: explicit debug session state (loaded program, current snapshot,
  run mode, break conditions, trace buffer)

## Initial Functional Requirements (High-Level)

1. Load emulator WASM and initialize a debug session in-browser.
2. Load a ROM/program image and reset to canonical boot state.
3. Execute one instruction at a time and surface resulting state changes.
4. Execute until pause/break condition and keep UI state synchronized.
5. Render register/memory/disassembly/fault data after each execution boundary.
6. Export/import session snapshots for deterministic replay workflows.

## Milestones (Draft)

1. M1: App scaffold (`Vite + Svelte + Tailwind`) and terminal-style layout.
2. M2: WASM adapter integration and basic run/step/reset controls.
3. M3: State inspectors (registers, memory, disassembly, faults/events).
4. M4: Snapshot/replay support and polish pass for v0.1.

## Risks and Mitigations

- WASM bridge complexity vs. UI responsiveness.
  - Mitigation: strict typed adapter boundary and incremental state updates.
- State volume causing render slowdowns.
  - Mitigation: windowed memory/disassembly views and batched redraw policy.
- Divergence from emulator-core semantics.
  - Mitigation: consume shared fixtures/traces from core conformance workflows.
