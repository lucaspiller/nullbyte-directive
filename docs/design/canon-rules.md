# Canon Rules

Canon only works if terms stay stable under pressure. In this setting, naming is
not cosmetic. A changed label can imply a changed mechanism, and a changed
mechanism can quietly break timeline, doctrine, and technical continuity.

This document sets the constraints for canonical writing in Nullbyte Directive
and explains why those constraints exist.

## Canon Starts with Fixed Names

Use `Nullbyte Directive` as the only project title in files under `docs/`. Do
not alternate between titles for tone or variety.

Core doctrine terms stay fixed and capitalized: `Exodus Protocol`,
`Laminated Stack`, `Quiet Burn`, and `Flight Core`. Layer labels are also
canonical and must be written exactly as `Layer 0: Hardwired Safeties`,
`Layer 1: Flight Core`, and `Layer 2: Peripheral and Comfort Compute`.

Do not rename stable architecture terms for stylistic effect. If a sentence
would call the Flight Core an "autopilot brain" or "ship computer," rewrite the
sentence instead.

## Tone Must Stay Operational

Canonical docs should read like engineering reality under political stress, not
mythic fiction and not product copy. Write in concrete system terms: interfaces,
failure modes, logistics, incentives, constraints, and tradeoffs.

Risk language should stay procedural and physical. Prefer contamination
boundaries, update-chain exposure, power and thermal limits, and delta-v costs
over abstract claims about danger.

If a mechanism is uncertain, mark it as uncertain. Canon is allowed to contain
open questions; it is not allowed to present speculation as settled fact.

Do not reference external projects, creators, or inspirations in canonical
material.

## Continuity Is a Cross-Document Contract

A canon edit is never local. Timeline, doctrine, and technical docs must remain
mutually true after every change.

The baseline anchors are fixed: Exodus Protocol ratification remains in 2063,
the Laminated Stack remains the shared trust model, and authoritative control
paths remain physically constrained.

Trust-boundary semantics are also fixed. `Layer 2` may advise but does not
directly actuate critical systems, and `Layer 0` protections are not overridable
by convenience software.

When introducing a new canonical term, define it once in the most relevant
document, then reuse that exact spelling everywhere else. If the term changes
how multiple files should read, backfill those files in the same editing pass.

## Editorial Gate Before Merge

Before finalizing a canon edit, run a term and continuity sweep.

- Confirm title usage is only `Nullbyte Directive`.
- Confirm core doctrine terms are capitalized canonically.
- Confirm new claims do not contradict `docs/in-game/canon/timeline.md`.
- Confirm technical claims do not violate
  `docs/in-game/technical/compute/compute-constraints.md`.
- Confirm world and gameplay implications stay aligned with
  `docs/in-game/canon/gameplay-pillars.md`.

If a change passes local readability but fails this sweep, it is not ready.
