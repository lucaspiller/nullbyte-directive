# emulator-core

`emulator-core` is the correctness-first deterministic virtual CPU
implementation used by Nullbyte Directive.

## Architecture

The crate is organized around strict phase boundaries:

- Decode and validation: `src/decoder.rs`, `src/encoding.rs`
- Execute and commit ordering: `src/execute/mod.rs`, `src/execute/flags.rs`
- Architectural state and run-state machine: `src/state/*`, `src/api.rs`
- Memory and region policy: `src/memory/*`
- Fault taxonomy and diagnostics model: `src/fault.rs`, `src/diag.rs`
- Timing model and cycle-cost table: `src/timing.rs`

The step pipeline preserves deterministic behavior by using a fixed decode path,
a fixed commit order, and boundary checks only at instruction boundaries.

## Host API Usage

Typical host integration path:

1. Build a state object (`CoreState::default()` or `CoreState::with_config`).
2. Load guest code/data into `state.memory`.
3. Provide an `MmioBus` implementation.
4. Execute with `step_one` (single-step) or `run_one`/`run_one_with_trace`
   (boundary stepping).
5. Save/restore deterministic snapshots through `CoreSnapshot`.

Key public surface area is re-exported from `src/lib.rs` for direct crate use.

## MMIO Contract

MMIO uses synchronous 16-bit operations through `MmioBus`:

- `read16(addr) -> Result<u16, MmioError>`
- `write16(addr, value) -> Result<MmioWriteResult, MmioError>`

Behavior guarantees:

- MMIO operations are strongly ordered with memory/MMIO side effects.
- Denied writes are suppressed without ISA fault and are counted in diagnostics.
- Adapter errors map to deterministic execution outcomes.

## Event Injection Contract

External events use a bounded deterministic FIFO (`EventQueueSnapshot`):

- Maximum queue capacity is 4 entries.
- Host injection uses `enqueue` with explicit overflow signaling.
- Dispatch occurs only at instruction boundaries when events are enabled.
- Same-input event streams produce stable dequeue order.

For replay, use `ReplayEventStream` with `replay_from_snapshot` or
`replay_with_trace`.
