# Emulator Core Spec Clarifications

- Last updated: 2026-02-13
- Source PRD: `docs/emulator/emulator-core-prd.md` (Draft v0.1)

## Purpose

This is the living appendix for ambiguous or underspecified emulator behavior.
Each entry captures the question, the current resolution (if any), and the
implementation/testing impact.

## Status Legend

- `Open`: ambiguity still unresolved; implementation should avoid assumptions.
- `Resolved`: decision accepted for `v0.1` and should be enforced in code/tests.
- `Deferred`: intentionally postponed beyond `v0.1`.

## Clarification Log

| ID     | PRD refs          | Status   | Clarification / Resolution                                                                                                                            | Impact                                                                                                          |
| ------ | ----------------- | -------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------- |
| SC-001 | FR-10             | Open     | Reset defaults list includes required registers but does not define exact numeric defaults for all registers (for example `SP`, `EVP`, and `R0..R7`). | Blocker for canonical reset conformance vectors; lock explicit reset constants before implementing FR-10 tests. |
| SC-002 | FR-5, FR-6, FR-15 | Open     | Tick budget is `640 cycles`, but threshold semantics need an exact rule: fault when `TICK + cost > 640` vs other boundary interpretation.             | Affects budget fault timing and whether the crossing instruction retires.                                       |
| SC-003 | FR-2              | Open     | Region legality requirements are explicit for fetch/write, but read behavior for reserved region `0xF100..0xFFFF` is not fully specified.             | Needed for deterministic read fault mapping and memory access tests.                                            |
| SC-004 | FR-2, FR-12       | Open     | DIAG window required fields are defined semantically, but field-to-address layout is not yet specified in this PRD.                                   | Required before DIAG read/write test vectors and snapshot compatibility checks.                                 |
| SC-005 | FR-7, FR-11       | Open     | Event queue overflow must fault deterministically, but exact timing (on enqueue vs boundary dispatch check) needs explicit statement.                 | Affects host API contract and event integration tests.                                                          |
| SC-006 | FR-6, FR-6A       | Open     | Multiple concurrent boundary conditions need precedence order: pending event dispatch, budget fault handling, and latched fault dispatch.             | Required for deterministic run-loop ordering and precise-fault guarantees.                                      |
| SC-007 | FR-13             | Open     | MMIO adapter errors must map to explicit deterministic outcomes, but the canonical mapping table (fault class vs rejection) is not in the PRD.        | Required for stable `MmioError` to `StepOutcome` behavior across adapters.                                      |
| SC-008 | FR-14             | Open     | Capability gating is required, but bit-to-feature mapping for optional instruction families is not enumerated in this PRD.                            | Needed to implement capability checks and non-authority conformance tests.                                      |
| SC-009 | Risks + Decisions | Resolved | For `v0.1`, this file is the normative tie-breaker for implementation-level ambiguity until PRD text is updated.                                      | Prevents drift between implementation and later documentation updates.                                          |

## Notes

- When an item is resolved, add: decision date, owner, and linked test IDs.
- PRD updates should reference the clarification ID they absorb, then mark the
  entry `Resolved` with the merged PRD revision.
