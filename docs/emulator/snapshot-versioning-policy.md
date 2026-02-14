# Emulator Core Snapshot Versioning Policy

This document defines snapshot migration and compatibility policy for
`emulator-core`.

## Scope

Snapshot policy applies to `CoreSnapshot` and `CanonicalStateLayout` exposed by
`crates/emulator-core/src/api.rs`.

## Versioning Rules

- Snapshot payloads are explicitly versioned by `SnapshotVersion`.
- `v0.1.x` supports wire version `V1` only.
- New snapshot wire versions are additive at crate minor/major boundaries.
- Existing snapshot versions are never silently reinterpreted.

## Compatibility Guarantees

- Patch releases (`0.1.x`) preserve backward compatibility for `V1` snapshots.
- Minor/major releases may add a new snapshot version while retaining explicit
  import behavior for older supported versions.
- Import rejects invalid payloads with deterministic `SnapshotLayoutError`
  variants.

## Migration Policy

When introducing `V{N+1}`:

1. Keep the previous version decoder available for at least one stable major
   line.
2. Add an explicit conversion path from `V{N}` to `V{N+1}`.
3. Add round-trip tests for both `V{N}` and `V{N+1}`.
4. Document field additions/removals and compatibility impact in release notes.

## Breaking Changes

A major-version bump is required when:

- A previous snapshot version is dropped from supported import paths.
- Deterministic meaning of an existing serialized field changes.

## Operational Guidance

- Hosts should persist the snapshot version with saved state metadata.
- Hosts should fail closed on unknown versions and request migration.
- CI should keep cross-host replay checks to detect nondeterministic drift.
