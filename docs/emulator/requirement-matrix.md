# Emulator Core Requirement Matrix

- Last updated: 2026-02-13
- Source PRD: `docs/emulator/emulator-core-prd.md` (Draft v0.1)
- Scope: traceability map from PRD requirements to implementation modules and
  tests in `crates/emulator-core`

## Usage

- `Planned` means target module/test paths are reserved but not implemented yet.
- Every new emulator-core implementation PR should update this matrix and link
  test IDs for touched rows.
- CI enforces this via `.github/workflows/traceability-gate.yml` for PRs that
  touch `crates/emulator-core` implementation/test/fuzz/bench files.

## Implemented Artifacts

- External host API contract scaffold (`CoreConfig`, `CoreState`, `StepOutcome`,
  `RunOutcome`, `MmioBus`, snapshot and trace surface types) is implemented in
  `crates/emulator-core/src/api.rs` with baseline contract tests and crate-root
  re-exports in `crates/emulator-core/src/lib.rs`. This artifact establishes
  typed integration boundaries for FR-8, FR-9, FR-11, FR-15, and NFR-4.
- Fault taxonomy scaffold (`FaultCode`, `FaultClass`) is implemented in
  `crates/emulator-core/src/fault.rs` with stable code mapping tests. This
  artifact establishes shared fault language for FR-2, FR-6, FR-13, and NFR-3.
- Cycle-cost table artifact (`CycleCostKind`, `CYCLE_COST_TABLE`, `cycle_cost`)
  is implemented in `crates/emulator-core/src/timing.rs` with table-consumer
  tests. This artifact establishes a single timing source of truth for FR-5.
- Opcode/encoding table artifact (`OpcodeEncoding`, `OPCODE_ENCODING_TABLE`,
  `classify_opcode`) is implemented in `crates/emulator-core/src/encoding.rs`
  with reserved/illegal classification tests. This artifact establishes a single
  decode source of truth for FR-3 and FR-4.
- Architectural register model scaffold (`ArchitecturalState`,
  `GeneralRegister`) is implemented in
  `crates/emulator-core/src/state/registers.rs` with coverage for all
  architectural registers. This artifact provides the FR-1 baseline state model
  for later FLAGS/CAP/EVP semantics and reset behavior.
- FLAGS semantics scaffold (`FLAGS_Z/N/C/V/I/F`, `FLAGS_ACTIVE_MASK`, masked
  `set_flags`, and bit-level helper methods) is implemented in
  `crates/emulator-core/src/state/registers.rs` with tests proving bits `6..15`
  are ignored on write and read as `0`. This artifact advances FR-1 conformance
  for status/control flag register behavior.
- CAP semantics scaffold (`CAP_AUTHORITY_DEFAULT_MASK`, default
  `ArchitecturalState` capability initialization, and read-only architectural
  `set_cap`) is implemented in `crates/emulator-core/src/state/registers.rs`
  with tests proving authority-default `CAP[0..3] = 1` and ignored
  architecturally visible writes. This artifact advances FR-1 and FR-14
  conformance for baseline capability behavior.
- EVP semantics scaffold (read-only architectural `set_evp` plus core-owned
  `set_evp_core_owned` update path) is implemented in
  `crates/emulator-core/src/state/registers.rs` with tests proving
  architecturally visible writes are ignored while core event-ownership updates
  remain effective. This artifact advances FR-1 conformance for event-pending
  register ownership behavior.
- Reset semantics scaffold (`CoreState::reset_canonical`) is implemented in
  `crates/emulator-core/src/api.rs` with tests proving canonical reset restores
  baseline architectural state (`PC=0x0000`, authority `CAP` defaults), clears
  event queue and fault latch state, and preserves loaded memory image. This
  artifact advances FR-10 conformance for reset/boot behavior.
- Dedicated FR-10 reset/boot integration coverage is implemented in
  `crates/emulator-core/tests/fr10_reset_boot.rs`, validating canonical reset
  register defaults, boot `PC`, queue/fault-latch clearing, profile-aware `CAP`
  defaults, and memory-image preservation across reset.
- Profile-aware capability scaffold (`CoreState::with_config`, profile-aware
  reset defaults, and `capability_enabled` helpers) is implemented in
  `crates/emulator-core/src/api.rs` and
  `crates/emulator-core/src/state/registers.rs` with tests covering authority
  and restricted profile CAP defaults plus core-owned capability updates. This
  artifact advances FR-14 conformance for capability-gating integration hooks.
- Run-state machine scaffold (`RunState` with explicit `Running`,
  `HaltedForTick`, `HandlerContext`, and `FaultLatched(FaultCode)` states) is
  implemented in `crates/emulator-core/src/state/run_state.rs` and integrated
  into `CoreState` in `crates/emulator-core/src/api.rs`, with tests proving
  deterministic fault-latch state transitions and reset clearing behavior. This
  artifact advances FR-15 conformance for host-visible run-state semantics.
- Canonical snapshot layout scaffold (`CanonicalStateLayout`,
  `SnapshotLayoutError`, and `CoreSnapshot` conversion helpers) is implemented
  in `crates/emulator-core/src/api.rs` with tests proving full-state round-trip
  determinism and strict rejection of invalid serialized payloads. This artifact
  advances FR-9 and NFR-1 conformance for serialization-safe deterministic
  snapshot state transfer.
- Memory map scaffold (`MemoryRegion`, fixed region boundary constants,
  `RegionDescriptor`, `FIXED_MEMORY_REGIONS`, `decode_memory_region`, and
  canonical `new_address_space` allocator) is implemented in
  `crates/emulator-core/src/memory/map.rs` and
  `crates/emulator-core/src/memory/mod.rs`, integrated by `CoreState` in
  `crates/emulator-core/src/api.rs`, with tests proving full 16-bit decode
  coverage and 64 KiB backing-store allocation. The fixed-region descriptor
  table is compile-time validated for exact PRD bounds and contiguous full-space
  coverage. This artifact advances FR-2 conformance for enforced fixed region
  layout and explicit address-space backing.
- Memory access-policy scaffold (`validate_fetch_access`,
  `validate_write_access`, `validate_word_alignment`, `validate_mmio_width`,
  `validate_mmio_alignment`) is implemented in
  `crates/emulator-core/src/memory/access.rs` and exported through
  `crates/emulator-core/src/memory/mod.rs` and
  `crates/emulator-core/src/lib.rs`, with tests proving deterministic
  region-legality and 16-bit alignment/width fault outcomes. This artifact
  advances FR-2 conformance for deterministic illegal
  fetch/write/misalignment/illegal-width behavior.
- Instruction decoder scaffold (`Decoder`, `DecodedInstruction`,
  `DecodedOrFault`, `AddressingMode`, `RegisterField`, `FaultReason`) is
  implemented in `crates/emulator-core/src/decoder.rs` and exported through
  `crates/emulator-core/src/lib.rs`, with tests proving exhaustive 16-bit decode
  classification (valid/reserved/illegal by policy), addressing mode validity
  (AM 000-101 valid, 110-111 fault), sign extension validation for AM=010,
  reserved opcode detection, and all valid opcode encodings decode correctly.
  This artifact advances FR-3 and FR-4 conformance for instruction decode
  validation and field extraction.

## Requirement Traceability

| ID    | PRD Requirement                                 | Implementation module(s) (planned)                                                                                                                                                                                                                              | Test file(s) (planned)                                                                                                                                   | Status      |
| ----- | ----------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------- |
| FR-1  | Architectural state and register semantics      | `crates/emulator-core/src/state/registers.rs`, `crates/emulator-core/src/state/flags.rs`, `crates/emulator-core/src/core.rs`                                                                                                                                    | `crates/emulator-core/tests/fr1_arch_state.rs`, `crates/emulator-core/tests/fr1_flags.rs`                                                                | In progress |
| FR-2  | Memory map and access policy                    | `crates/emulator-core/src/memory/map.rs`, `crates/emulator-core/src/memory/mod.rs`, `crates/emulator-core/src/memory/access.rs`, `crates/emulator-core/src/fault.rs`                                                                                            | `crates/emulator-core/src/memory/map.rs`                                                                                                                 | In progress |
| FR-3  | Instruction set support and semantic edge rules | `crates/emulator-core/src/encoding.rs`, `crates/emulator-core/src/decoder.rs`, `crates/emulator-core/src/execute/control.rs`, `crates/emulator-core/src/execute/alu.rs`, `crates/emulator-core/src/execute/math.rs`, `crates/emulator-core/src/execute/data.rs` | `crates/emulator-core/tests/fr3_opcode_semantics.rs`, `crates/emulator-core/tests/fr3_flags_table.rs`, `crates/emulator-core/tests/fr3_div_mod_zero.rs`  | In progress |
| FR-4  | Addressing modes and AM validation              | `crates/emulator-core/src/encoding.rs`, `crates/emulator-core/src/decoder.rs`, `crates/emulator-core/src/execute/effective_address.rs`                                                                                                                          | `crates/emulator-core/tests/fr4_addressing_modes.rs`, `crates/emulator-core/tests/fr4_am010_sign_ext.rs`                                                 | In progress |
| FR-5  | Deterministic timing model and budget           | `crates/emulator-core/src/timing.rs`, `crates/emulator-core/src/execute/retire.rs`                                                                                                                                                                              | `crates/emulator-core/tests/fr5_cycle_costs.rs`, `crates/emulator-core/tests/fr5_budget_fault.rs`                                                        | Planned     |
| FR-6  | Dispatch and fault semantics                    | `crates/emulator-core/src/dispatch.rs`, `crates/emulator-core/src/fault.rs`, `crates/emulator-core/src/execute/control.rs`                                                                                                                                      | `crates/emulator-core/tests/fr6_dispatch.rs`, `crates/emulator-core/tests/fr6_handler_context.rs`, `crates/emulator-core/tests/fr6_double_fault.rs`      | Planned     |
| FR-6A | Commit order contract                           | `crates/emulator-core/src/execute/commit.rs`, `crates/emulator-core/src/execute/pipeline.rs`                                                                                                                                                                    | `crates/emulator-core/tests/fr6a_commit_order.rs`, `crates/emulator-core/tests/fr6a_precise_faults.rs`                                                   | In progress |
| FR-7  | Event queue behavior                            | `crates/emulator-core/src/event_queue.rs`, `crates/emulator-core/src/dispatch.rs`                                                                                                                                                                               | `crates/emulator-core/tests/fr7_event_queue.rs`, `crates/emulator-core/tests/fr7_overflow.rs`                                                            | Planned     |
| FR-8  | MMIO contract and ordering                      | `crates/emulator-core/src/api.rs`, `crates/emulator-core/src/mmio.rs`, `crates/emulator-core/src/execute/memory_ops.rs`, `crates/emulator-core/src/execute/control.rs`                                                                                          | `crates/emulator-core/tests/fr8_mmio_contract.rs`, `crates/emulator-core/tests/fr8_ordering_sync.rs`                                                     | In progress |
| FR-9  | Snapshot and replay primitives                  | `crates/emulator-core/src/api.rs`, `crates/emulator-core/src/snapshot.rs`, `crates/emulator-core/src/replay.rs`, `crates/emulator-core/src/core.rs`                                                                                                             | `crates/emulator-core/tests/fr9_snapshot_roundtrip.rs`, `crates/emulator-core/tests/fr9_replay_determinism.rs`                                           | In progress |
| FR-10 | Reset and boot semantics                        | `crates/emulator-core/src/api.rs`, `crates/emulator-core/src/reset.rs`, `crates/emulator-core/src/core.rs`, `crates/emulator-core/src/state/capabilities.rs`                                                                                                    | `crates/emulator-core/tests/fr10_reset_boot.rs`                                                                                                          | In progress |
| FR-11 | External event injection contract               | `crates/emulator-core/src/api.rs`, `crates/emulator-core/src/event_queue.rs`                                                                                                                                                                                    | `crates/emulator-core/tests/fr11_event_injection.rs`                                                                                                     | In progress |
| FR-12 | DIAG window semantics                           | `crates/emulator-core/src/diag/mod.rs`, `crates/emulator-core/src/diag/provider.rs`, `crates/emulator-core/src/fault.rs`                                                                                                                                        | `crates/emulator-core/tests/fr12_diag_window.rs`, `crates/emulator-core/tests/fr12_diag_saturation.rs`                                                   | Planned     |
| FR-13 | MMIO authorization semantics                    | `crates/emulator-core/src/mmio.rs`, `crates/emulator-core/src/diag/mod.rs`, `crates/emulator-core/src/fault.rs`                                                                                                                                                 | `crates/emulator-core/tests/fr13_mmio_authorization.rs`                                                                                                  | Planned     |
| FR-14 | Capability-bit enforcement                      | `crates/emulator-core/src/api.rs`, `crates/emulator-core/src/state/registers.rs`, `crates/emulator-core/src/state/capabilities.rs`, `crates/emulator-core/src/execute/capability_checks.rs`                                                                     | `crates/emulator-core/tests/fr14_capability_gating.rs`                                                                                                   | In progress |
| FR-15 | Run-state and boundary semantics                | `crates/emulator-core/src/api.rs`, `crates/emulator-core/src/state/run_state.rs`, `crates/emulator-core/src/timing.rs`, `crates/emulator-core/src/execute/control.rs`                                                                                           | `crates/emulator-core/tests/fr15_ewait_halt.rs`, `crates/emulator-core/tests/fr15_boundary_transitions.rs`                                               | In progress |
| NFR-1 | Determinism                                     | `crates/emulator-core/src/core.rs`, `crates/emulator-core/src/replay.rs`, `crates/emulator-core/src/timing.rs`                                                                                                                                                  | `crates/emulator-core/tests/nfr1_determinism_cross_run.rs`, `crates/emulator-core/tests/nfr1_determinism_ci.rs`                                          | Planned     |
| NFR-2 | Performance envelope (post-v0.1)                | `crates/emulator-core/benches/step_throughput.rs`                                                                                                                                                                                                               | `crates/emulator-core/tests/nfr2_perf_smoke.rs`                                                                                                          | Planned     |
| NFR-3 | Safety and correctness                          | `crates/emulator-core/src/fault.rs`, `crates/emulator-core/src/decode/decoder.rs`, `crates/emulator-core/fuzz/fuzz_targets/*`                                                                                                                                   | `crates/emulator-core/tests/nfr3_no_panic.rs`, `crates/emulator-core/tests/nfr3_illegal_faults.rs`, `crates/emulator-core/tests/nfr3_property_memory.rs` | Planned     |
| NFR-4 | Inspectability and trace stability              | `crates/emulator-core/src/api.rs`, `crates/emulator-core/src/trace.rs`                                                                                                                                                                                          | `crates/emulator-core/tests/nfr4_trace_hooks.rs`, `crates/emulator-core/tests/nfr4_trace_golden.rs`                                                      | In progress |

## Acceptance Criteria Traceability

| ID   | PRD Acceptance Criterion                                                                      | Primary test file(s) (planned)                                                                                                                               | Linked requirement rows |
| ---- | --------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------ | ----------------------- |
| AC-1 | Implemented instructions pass conformance vectors and edge-case tests                         | `crates/emulator-core/tests/fr3_opcode_semantics.rs`, `crates/emulator-core/tests/fr3_flags_table.rs`, `crates/emulator-core/tests/conformance_vectors.rs`   | FR-3, FR-4, FR-6A       |
| AC-2 | Deterministic replay passes across at least two macOS/Linux CI runners                        | `crates/emulator-core/tests/fr9_replay_determinism.rs`, `crates/emulator-core/tests/nfr1_determinism_ci.rs`                                                  | FR-9, NFR-1             |
| AC-3 | Tick budget and fault semantics match canonical behavior                                      | `crates/emulator-core/tests/fr5_budget_fault.rs`, `crates/emulator-core/tests/fr6_dispatch.rs`, `crates/emulator-core/tests/fr15_boundary_transitions.rs`    | FR-5, FR-6, FR-15       |
| AC-4 | No guest input crashes host process under fuzz stress budget                                  | `crates/emulator-core/fuzz/fuzz_targets/decode.rs`, `crates/emulator-core/fuzz/fuzz_targets/execute.rs`, `crates/emulator-core/tests/nfr3_no_panic.rs`       | NFR-3                   |
| AC-5 | Reset/boot, DIAG latching, and precise-fault behavior pass dedicated conformance scenarios    | `crates/emulator-core/tests/fr10_reset_boot.rs`, `crates/emulator-core/tests/fr12_diag_window.rs`, `crates/emulator-core/tests/fr6a_precise_faults.rs`       | FR-10, FR-12, FR-6A     |
| AC-6 | Capability gating, EWAIT/HALT semantics, and handler-context fault semantics pass conformance | `crates/emulator-core/tests/fr14_capability_gating.rs`, `crates/emulator-core/tests/fr15_ewait_halt.rs`, `crates/emulator-core/tests/fr6_handler_context.rs` | FR-14, FR-15, FR-6      |
