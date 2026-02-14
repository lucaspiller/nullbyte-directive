# emulator-core v0.1.0 Release Notes

Release date: 2026-02-14

## Summary

`emulator-core v0.1.0` delivers the correctness-first deterministic core for
Nullbyte Directive with full ISA decode/execute scaffolding, deterministic
replay, bounded event queue semantics, MMIO integration contracts, and
conformance-oriented tests.

## Included in v0.1.0

- Deterministic decode, execute, commit-order, and boundary-step APIs.
- Canonical memory map enforcement and diagnostics window support.
- Fault taxonomy and deterministic fault dispatch behavior.
- Snapshot import/export and replay surface with explicit versioning.
- Optional trace sink for stable diff-based debugging output.
- Wasm bridge crate (`emulator-wasm`) integration and smoke coverage.
- Fuzz/property/stress-oriented hardening tests and CI determinism checks.

## Known Non-Goals

- Performance envelope optimization for large-scale core counts.
- First-party production adapters beyond the core wasm bridge.
- Runtime memory map remapping or dynamic ISA extension loading.
- Device-rich simulation beyond the defined MMIO contract boundary.

## Post-v0.1 Plan

- Phase 15 performance harness and optimization work.
- Extended adapter integration path for server runtime bindings.
- Additional coverage depth for long-running fuzz campaigns in CI.
- Snapshot schema evolution with explicit migration paths when needed.

## Upgrade and Compatibility Notes

- Snapshot wire format is `V1` for the `0.1.x` line.
- Consumers should treat unknown snapshot versions as unsupported and fail
  closed.
- Determinism validation now includes cross-host fingerprint comparison in CI
  (Linux vs macOS).
