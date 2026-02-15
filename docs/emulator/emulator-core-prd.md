# Emulator Core PRD

## Document Status

- Owner: Core Systems
- Last Updated: 2026-02-13
- Status: Draft v0.1

## Context

`emulator-core` is the Rust reference implementation target for the Nullbyte One
authority CPU simulation used by Nullbyte Directive services.

This PRD is derived from existing project canon and technical constraints:

- `docs/in-game/technical/compute/nullbyte-one-core.txt`
- `docs/in-game/technical/compute/compute-constraints.txt`
- `docs/design/technical/nullbyte-one.md`
- `docs/in-game/canon/doctrine.md`
- `docs/in-game/canon/gameplay-pillars.md`

The core requirement is not peak throughput. The core requirement is a
predictable, inspectable, deterministic execution model that can be trusted in
authority-path simulation and scaled across many concurrent modules.

## Problem Statement

We need one reusable CPU core implementation that:

- Executes the Nullbyte One ISA exactly and deterministically.
- Preserves fixed timing behavior (cycle cost and tick budget semantics).
- Enforces memory and fault rules from the specification.
- Is embeddable in multiple runtimes:
  - Server-side simulation (authoritative runtime).
  - Browser debug tooling via a separate `wasm` adapter crate.

Without a clean `emulator-core`, we risk divergent behavior between server and
browser tools, inconsistent debugging, and brittle integration boundaries.

## Product Goals

1. Spec-correct execution of the Nullbyte One core.
2. Deterministic behavior across machines and builds for identical inputs.
3. Stable, small API surface suitable for adapters (`node`, `wasm`, CLI).
4. High testability: behavior-level tests tied directly to spec sections.
5. Complete correctness-focused `v0.1` release with full conformance coverage.

## Non-Goals (For This PRD)

1. Implementing the wasm packaging layer (`emulator-wasm`).
2. Implementing Node native bindings (`napi-rs`) or process orchestration.
3. Implementing full in-game devices beyond core MMIO contract stubs.
4. Building visual debugging UI.
5. Simulating triple-core voter internals (server runs one reference core).

## Target Users

1. Server runtime engineers embedding the core in authority simulation services.
2. Tooling engineers building debug and inspection interfaces.
3. Content/systems engineers authoring ROM programs and validation fixtures.

## Scope

### In Scope

- CPU state model (registers, flags, core status).
- 64 KiB memory model with fixed region policies.
- Instruction decode + execute pipeline for defined ISA opcodes.
- Deterministic cycle accounting and tick budget behavior.
- Trap/event/fault dispatch semantics and vector handling.
- Minimal MMIO interface contract for external device emulation.
- Serialization/snapshot primitives for save/load and deterministic replay.

### Out of Scope

- Networking, persistence infrastructure, and multiplayer protocols.
- Scenario scripting systems.
- UI/UX for debugger tools.
- Non-Rust runtime adapters (implemented in other crates).

## Functional Requirements

### FR-1: Architectural State

The crate must model the full architectural register set:

- `R0..R7`, `PC`, `SP`, `FLAGS`, `TICK`, `CAP`, `CAUSE`, `EVP`.

`FLAGS` behavior must match spec bit semantics for `Z`, `N`, `C`, `V`, `I`, and
`F`, with bits `6..15` reading as `0` and ignored on write.

`CAP` and `EVP` are read-only architecturally, with `CAP[0..3] = 1` for the
authority profile default.

### FR-2: Memory Map and Access Policy

The core must enforce the fixed 16-bit address space regions:

- ROM: `0x0000..0x3FFF`
- RAM: `0x4000..0xDFFF`
- MMIO: `0xE000..0xEFFF`
- DIAG: `0xF000..0xF0FF`
- Reserved: `0xF100..0xFFFF`

Fetch/write legality by region must follow the spec, including fault behavior
for illegal accesses and reserved addressing.

Memory representation and access rules must also be explicit:

- Big-endian word semantics for memory-visible multi-word values.
- 16-bit access alignment and width rules as defined by the spec.
- Deterministic fault outcomes for misaligned or illegal-width accesses.

### FR-3: Instruction Set Support

The core must implement all defined opcode classes and sub-operations in the
current spec revision, including:

- Control ops (`NOP`, `SYNC`, `HALT`, `TRAP`, `SWI`).
- Data movement (`MOV`, `LOAD`, `STORE`).
- Integer ALU (`ADD`, `SUB`, `AND`, `OR`, `XOR`, `SHL`, `SHR`, `CMP`).
- Math helpers (`MUL`, `MULH`, `DIV`, `MOD`, `QADD`, `QSUB`, `SCV`).
- Branch/jump, stack/call, event and fault control classes per spec.

Reserved encodings must produce illegal-encoding fault behavior.

Decoder and semantic edge rules from spec section 6/7 are mandatory:

- Unused register fields must be `000`; non-zero unused fields are illegal
  encoding faults.
- `DIV`/`MOD` divide-by-zero behavior is deterministic: if `B == 0`, set
  `R[RD] = 0` and do not fault.
- Per-instruction FLAGS behavior must match the section 7 table exactly,
  including instructions that clear `C/V` and instructions that do not modify
  FLAGS.

### FR-4: Addressing Modes

Addressing modes `000..101` must be implemented exactly per effective-address
rules. `110` and `111` must fault.

The `AM=010` sign-extension consistency rule must be enforced:

- Extension low byte carries signed `disp8`.
- Extension high byte must be sign-copy (`0x00` or `0xFF`) or fault.

### FR-5: Deterministic Timing Model

Instruction cycle costs must be fixed and data-independent.

The core must:

- Increment `TICK` after instruction commit by instruction cost.
- Enforce tick budget of `640 cycles`.
- Raise budget fault semantics as defined when threshold is crossed.

Timing behavior must be deterministic across hosts for identical initial state
and input events.

### FR-6: Dispatch and Fault Semantics

The core must implement vector-based dispatch for `TRAP`, `EVENT`, and `FAULT`
with correct state latching (`CAUSE`, `R0`, stack push order, event disable) and
`ERET` semantics.

Budget fault special handling (halt remainder of tick, next-tick resume/fault
policy) must match the design brief behavior.

Dispatch/fault behavior must be "precise" at instruction boundaries:

- Architected state reflects either fully committed instruction effects or none.
- Fault handling order and visible side effects are deterministic and testable.

Fault escalation rules must match section 12:

- `ERET` faults when executed outside handler context.
- If `VEC_FAULT` is invalid or a double-fault occurs during fault handling, the
  core halts.

### FR-6A: Commit Order Contract

Instruction retirement must follow the exact section 7 commit sequence:

1. Read source operands
2. Compute result and/or effective address
3. Perform memory/MMIO reads
4. Perform memory/MMIO writes
5. Write destination register
6. Update FLAGS
7. Advance PC

This order is binding for correctness and fault behavior.

### FR-7: Event Queue

Provide bounded 4-entry FIFO event queue behavior with:

- Boundary checks only at instruction boundaries when `FLAGS.I == 1`.
- Overflow fault behavior.
- Deterministic dequeue ordering.

### FR-8: MMIO Contract

`emulator-core` must expose an abstract MMIO interface (trait) with
deterministic read/write behavior and explicit error/fault mapping.

This allows server and wasm adapters to plug different device models without
forking CPU semantics.

Ordering guarantees are mandatory:

- MMIO operations are strongly ordered relative to memory and MMIO operations.
- `OUT` side effects become visible at instruction commit.
- `SYNC` enforces visibility of prior memory/MMIO effects before next
  instruction execution.
- Async device backends, if used by adapters, must preserve this ordering as-if
  operations were executed synchronously in commit order.

### FR-9: Snapshots and Replay

The crate must expose snapshot import/export primitives sufficient for:

- Save/restore of full architectural state + memory image.
- Deterministic replay test harnesses.

Snapshot format may evolve, but must be versioned and backward-safe within major
version.

### FR-10: Reset and Boot Semantics

The core must implement canonical reset state and boot entry behavior,
including:

- Reset register values (`PC`, `SP`, `FLAGS`, `TICK`, `CAUSE`, `EVP`, `R0..R7`).
- Authority-profile capability defaults (`CAP` bits).
- First fetch at ROM entry (`PC=0x0000`) with no hidden startup path.
- Reset clears event queue and any latched fault state.

### FR-11: External Event Injection Contract

The host-facing API must define deterministic event enqueue semantics:

- Explicit enqueue/dequeue APIs for 8-bit event IDs.
- Bounded queue behavior and overflow signaling mapped to core fault semantics.
- Stable ordering guarantees for same-tick multi-event injection.

### FR-12: Diagnostics Window Semantics

The DIAG region contract must expose the section 14 core-owned fields:

- Last fault code
- Last faulting `PC`
- Last fault tick index
- Per-class fault counters (saturating 16-bit)
- Executed-instruction counter (saturating 16-bit)

Provider pluggability must not weaken required core-owned diagnostic fields.

### FR-13: MMIO Authorization Semantics

MMIO writes requiring external authorization must have explicit, deterministic
integration behavior:

- Host adapter can return authorized/denied/error outcomes.
- Denied writes are silently suppressed (no ISA fault) and recorded for
  diagnostics.
- Adapter/hardware errors map to explicit fault or rejection outcomes.
- No adapter-specific behavior may change ISA-visible results.

### FR-14: Capability-Bit Enforcement

The emulator must define behavior for profiles where capability bits are not
set:

- Optional instruction families and hardware features are gated by `CAP`.
- Executing a gated feature when disabled must produce deterministic fault
  behavior.
- Authority profile remains default (`CAP[0..3] = 1`), but non-authority profile
  behavior is test-covered to prevent future regressions.

### FR-15: Run-State and Boundary Semantics

The host API must define deterministic behavior for control-flow wait states:

- `EWAIT` semantics follow section 7 exactly:
  - If event queue empty, `PC` remains at `EWAIT` (instruction re-executes).
  - If non-empty, `PC` advances to next instruction.
  - `EWAIT` still costs 1 cycle per execution and participates in budget checks.
- `HALT` semantics follow section 7:
  - `HALT` retires with cost 1 and advances `PC` to `PC_next`.
  - The core then enters halted state for the remainder of the current tick.
  - Event arrival does not wake the core mid-tick.
  - At next tick boundary, core resumes from current `PC` with fresh budget.
- Interaction with tick accounting and dispatch checks at boundaries is
  deterministic and test-covered.

## Non-Functional Requirements

### NFR-1: Determinism

- No dependence on wall-clock time, randomness, thread scheduling, or host
  endianness.
- Same input sequence and initial snapshot must produce byte-identical output
  state and fault/event sequence.

### NFR-2: Performance Envelope

Performance scaling is a follow-up phase target (post-`v0.1`), not a release
gate for core correctness.

Follow-up target:

- Baseline hardware profile: Apple M3 Pro (16 GB), with 4 performance cores
  allocated to simulation.
- Ship target at 100% activity: >= 3,000 emulated cores at 100 Hz.
- Stretch target at 100% activity: >= 4,000 emulated cores at 100 Hz.

These targets assume batched stepping APIs and no debug tracing in the hot path.

### NFR-3: Safety and Correctness

- All illegal encodings and illegal memory accesses fault explicitly.
- No panics for guest-program-triggered behavior; return structured fault
  outcomes.
- Fuzzing and property tests for decode and memory safety boundaries.

### NFR-4: Inspectability

- Core step API must support optional trace hooks (instruction, PC, cycle,
  memory access, fault cause).
- Trace output must be deterministic and stable enough for diff-based debugging.

## Proposed Crate Boundary

### `crates/emulator-core`

Responsibilities:

- Spec execution engine.
- Memory + register model.
- Dispatch/event/fault machinery.
- MMIO trait boundary.
- Snapshot and deterministic stepping APIs.

Not responsible for:

- wasm bindings.
- JS-facing API ergonomics.
- Server process lifecycle.

## API Draft (Illustrative)

```rust
pub struct CoreConfig { /* caps, profile toggles, limits */ }
pub struct CoreState { /* registers, memory, queue, latched status */ }

pub trait MmioBus {
    fn read16(&mut self, addr: u16) -> Result<u16, MmioError>;
    fn write16(
        &mut self,
        addr: u16,
        value: u16,
    ) -> Result<MmioWriteResult, MmioError>;
}

pub enum MmioWriteResult {
    Applied,
    DeniedSuppressed,
}

pub enum StepOutcome {
    Retired { cycles: u16 },
    Halted,
    Fault { cause: FaultCause },
    Trap { cause: u16 },
    EventDispatch { event_id: u8 },
}

pub fn step_one(core: &mut CoreState, mmio: &mut dyn MmioBus) -> StepOutcome;
pub fn run_until_boundary(/* tick or halt boundary */) -> RunOutcome;
```

Final API can differ, but must preserve testability, deterministic behavior, and
adapter friendliness.

## Test Strategy

### Unit Tests

- Per-opcode semantics.
- Flag transitions.
- Addressing mode effective-address and fault cases.
- Cycle accounting by opcode.

### Spec Conformance Tests

- Table-driven test vectors mapped to spec sections.
- Golden traces for known ROM programs.
- Cross-implementation fixture compatibility tests (future SOA/BIC/HLS harness).

### Property and Fuzz Tests

- Decoder robustness for arbitrary 16-bit words.
- Memory access boundary conditions.
- Snapshot round-trip invariants.

### Integration Tests

- Tick budget overrun behavior.
- Event queue overflow and dispatch ordering.
- MMIO authorization/error propagation.
- Reset/boot register defaults and first-instruction fetch behavior.
- Precise-fault behavior (no partial commit) for illegal accesses/encodings.
- DIAG latch contents for canonical fault scenarios.
- Capability-gated opcode behavior for non-authority profiles.
- `EWAIT` re-execution semantics and budget interaction on empty queue.
- `ERET` fault when outside handler context.
- Double-fault and invalid `VEC_FAULT` halt behavior.
- Decoder enforcement for unused-register-field non-zero encodings.
- `DIV`/`MOD` divide-by-zero behavior (`R[RD]=0`, no fault).
- FLAGS conformance for all instruction classes.
- MMIO write deny path is silent suppression with DIAG latch update.
- MMIO strong ordering and `SYNC` visibility semantics.
- `HALT` remainder-of-tick halt and next-tick resume semantics.

## Milestones

1. M1: Core skeleton + register/memory model + minimal step loop.
2. M2: Full decode/execute for base opcodes + cycle accounting.
3. M3: Dispatch, fault model, and event queue semantics.
4. M4: Snapshot/replay + trace hooks.
5. M5: Conformance suite + fuzz/property tests.
6. M6: Release `emulator-core v0.1.0` (correctness-first, core crate only).

## Follow-Up Phase (Post-v0.1)

1. P2-M1: Performance harness and baseline measurement on target hardware.
2. P2-M2: Throughput optimization pass toward 3,000/4,000 targets.
3. P2-M3: First-party adapter integration (`emulator-wasm`, server in-process
   binding path).

## Acceptance Criteria

1. All implemented instructions pass conformance vectors and edge-case tests.
2. Deterministic replay test passes across at least two different macOS/Linux CI
   runners.
3. Tick budget and fault semantics match documented behavior for canonical
   scenarios.
4. No guest input can crash the host process under fuzz stress budget.
5. Reset/boot, DIAG latching, and precise-fault behavior pass dedicated
   conformance scenarios.
6. Capability gating, EWAIT/HALT semantics, and handler-context fault semantics
   pass conformance tests.

## Risks and Mitigations

- Ambiguity between in-game spec and implementation detail.
  - Mitigation: maintain a living "spec clarification" appendix in this folder.
- Early API lock-in blocking wasm/server ergonomics.
  - Mitigation: stabilize core semantics before freezing public API.
- Performance regressions from heavy tracing.
  - Mitigation: tracing hooks are opt-in and zero-cost when disabled.

## Decisions (v0.1)

1. DIAG modeling:
   - `DIAG` (`0xF000..0xF0FF`) uses a pluggable provider trait from day one.
   - `emulator-core` ships a default `StaticDiagProvider` for simple setups.
2. Time-travel debugging:
   - Instruction-level reversible execution is out of scope for `v0.1.x`.
   - Debug rewind is snapshot-based plus deterministic replay.
3. Server integration path:
   - Phase one uses direct in-process binding (Rust FFI/N-API path), not a
     subprocess CLI transport.
   - A CLI may exist as a developer harness, but it is not the primary server
     integration architecture.

## Dependencies

- Rust stable toolchain managed by `rustup`.
- Planned crate stack (subject to refinement):
  - `thiserror` for error typing.
  - `proptest` for property tests.
  - `arbitrary`/`libfuzzer` or equivalent fuzz harness tooling.

## Appendix: Capacity Estimate (Off-the-Shelf)

This estimate sets an initial target before implementation benchmarks exist.

Assumptions:

- Available host budget: 4 performance cores.
- Effective host frequency budget: ~16e9 host-cycles/s total.
- Guest load per emulated core at full activity:
  - 640 cycles/tick \* 100 ticks/s = 64,000 guest-cycles/s.
- Estimated average instruction cost: ~1.7 guest-cycles/instruction.
  - 64,000 / 1.7 = ~37,650 guest-instructions/s/core.

Capacity model:

- Host instructions budget = host-cycles/s / (host-cycles per guest-instruction)
- Emulated cores = host instructions budget / guest-instructions/s/core

Reference points:

- 150 host-cycles/guest-instr -> ~2,800 cores
- 100 host-cycles/guest-instr -> ~4,250 cores
- 60 host-cycles/guest-instr -> ~7,100 cores

Target selection:

- Use 3,000 as ship target to preserve operational headroom.
- Use 4,000 as stretch target with optimization and favorable workload shape.

## Appendix: Requirement Traceability

- ISA, memory, timing, dispatch:
  `docs/in-game/technical/compute/nullbyte-one-core.txt`
- Authority compute constraints:
  `docs/in-game/technical/compute/compute-constraints.txt`
- Simulation and budget profile intent: `docs/design/technical/nullbyte-one.md`
- Layer boundaries and trust model: `docs/in-game/canon/doctrine.md`
- Gameplay pressure alignment for compute scarcity:
  `docs/in-game/canon/gameplay-pillars.md`
