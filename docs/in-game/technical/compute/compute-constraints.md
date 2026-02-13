AUTHORITY-PATH COMPUTE CONSTRAINTS Exodus Protocol Compliance Document --
Revision 1.0

================================================================================
PREAMBLE
================================================================================

This document defines the compute constraints for any controller that can
influence life-critical actuation through an authority path. The Flight Core,
subsystem controllers, and anything else that can open a valve, fire a thruster,
or gate a safety interlock must meet these requirements.

The constraints exist to prevent Quiet Burn class failures -- systems that
optimise themselves past the point where humans can inspect, predict, or
override their behavior. Authority-path compute is intentionally impoverished.
If your code cannot fit within these limits, it belongs in advisory compute, not
on the authority path.

================================================================================
AT-A-GLANCE
================================================================================

- Fixed memory map, no runtime remapping
- No dynamic allocation, no unbounded recursion
- Fixed-point arithmetic only (no floating-point)
- All buffers declared at build time with fixed sizes
- Saturating arithmetic preferred over wraparound
- Explicit fault handling for every failure class
- Watchdog-enforced cycle budgets with deterministic fallback

================================================================================

1. # MEMORY MODEL

Authority-path controllers use a fixed memory map with a bounded address space.
The map is set at build time and cannot change during operation. Runtime
remapping is forbidden because it would allow a compromised controller to hide
state from inspection.

The canonical authority core uses a 16-bit address space (64 KiB total),
partitioned into fixed regions:

1. ROM / sealed kernel: boot code, safety logic, invariants
2. Program region: approved operations routines
3. RAM state: mode flags, timers, interlock state
4. Buffer region: bounded queues and ring buffers
5. MMIO region: register windows to hardware interfaces

Exact addresses are defined per core specification (see the Nullbyte One Core
Specification for the reference layout).

Allocation rules:

- No heap allocator in authority paths. Every byte of memory has a known purpose
  at build time.
- No unbounded recursion. Call depth is statically bounded.
- All buffers are fixed-size and declared at build time.
- Queue depth and record width are schema-defined and versioned. No negotiation
  at runtime.
- Every inter-core mailbox has explicit producer/consumer ownership. No
  shared-write buffers.

Data lifetime rules:

- Safety-critical state uses monotonic sequence IDs or time tags. You can always
  tell which value is newer.
- Cross-core payloads are read through snapshot or equivalent atomicity control.
  Torn reads are not acceptable.
- Stale data must be explicitly detectable and rejectable.
- Log storage uses bounded ring buffers with deterministic overwrite policy. Old
  entries are overwritten, never leaked.

Memory safety:

- Out-of-range index or length values are hard faults.
- Pointer arithmetic is minimised; table-indexed access is preferred.
- Integrity checks are required at trust boundaries: length, mode, version, and
  field legality.

================================================================================ 2)
FIXED-POINT ARITHMETIC
================================================================================

Floating-point is excluded from authority-path logic. This is is a safety
requirement. Floating-point introduces rounding-mode differences, NaN
propagation, and denormal behavior that varies between implementations. In a
system where three independent cores must agree on every result, those
differences would cause voter disagreements and false faults.

Fixed-point arithmetic is deterministic, testable, and auditable. It costs more
programmer effort, but it eliminates an entire class of certification problems.

Encoding rules:

- Signed fixed-point formats are declared per field family.
- Scale factors are fixed by schema version and never negotiated at runtime.
- Unit systems are closed and explicit (e.g. mission-time ticks, thrust command
  quanta, pressure counts).
- Mixed-scale operations require explicit conversion routines with saturation
  and bounds checks.

Numeric operation rules:

- Saturating arithmetic is preferred over wraparound for safety-relevant values.
  Silent overflow has killed people.
- Multiply paths must define intermediate width and overflow handling. A 16x16
  multiply produces a 32-bit intermediate; decide which half you need before you
  write the code.
- Division is available but expensive. Lookup tables and bounded approximations
  are preferred where precision allows.
- Equality checks on derived numeric values are avoided in gating logic. Use
  threshold bands instead -- two fixed-point values will rarely be exactly
  equal.

Offload boundary:

Heavy estimation, trajectory planning, and optimisation may run in advisory
compute (non-authority systems). But authority layers accept only bounded
artifacts -- targets, plans, constraints -- and re-validate them before
execution. The authority path never trusts advisory output blindly.

================================================================================ 3)
FAILURE MODES AND REQUIRED RESPONSES
================================================================================

Authority-path code must handle every failure class explicitly. "It probably
won't happen" is not an engineering argument on a ship where the nearest help is
light-minutes away.

Memory exhaustion or buffer overrun:

Indicators: - Queue length exceeds declared cap - Log ring advances faster than
policy budget - Mailbox write attempts on full buffer

Response: - Reject new non-critical payloads - Preserve interlock and inhibit
processing priority - Raise fault line and enter degrade mode if the condition
persists beyond threshold

Schema or bounds violation at input boundary:

Indicators: - Invalid field length - Unknown enum outside supported version -
Declared payload size exceeding contract

Response: - Reject the transaction - Latch incompatibility fault - Keep
last-known-valid control state; never execute a partially validated payload

Numeric overflow, underflow, or invalid scaling:

Indicators: - Saturation hit counters exceed policy threshold - Conversion
routine range fault - Intermediate width overflow detection

Response: - Mark computation result invalid - Block dependent actuation path -
Transition to conservative fallback (hold, abort, or safe-mode sequence)

Timing overrun and watchdog breach:

Indicators: - Cycle budget exceeded - Handshake timeout on required bus
transactions - Missed heartbeat or stale time tag

Response: - Invalidate stale advisory data - Revert to last accepted safe
control set - Trigger subsystem reset/re-qualify workflow when policy requires

State incoherence (torn or mixed snapshot):

Indicators: - Snapshot ID mismatch before/after read - Time tag regression -
Cross-field consistency checks fail

Response: - Discard read set - Retry snapshot cycle within bounded attempts -
Escalate to fault latch and degrade mode on repeated failure

Latent corruption or software integrity fault:

Indicators: - Boot-time integrity check failure - Runtime invariant breach in
protected region - Unexpected control-flow trap

Response: - Enter fail-safe execution profile - Revoke non-essential authority
outputs - Require maintenance-safe restart and re-certification before returning
to full mode

================================================================================ 4)
COMPLIANCE CHECKLIST
================================================================================

An authority-path implementation is compliant only if all of the following are
true:

- Address space and memory map are fixed and documented.
- No dynamic memory allocation in authoritative code.
- Fixed-point formats and scales are versioned and immutable in operation.
- Overflow and bounds handling are explicit and test-covered.
- Input contracts are validated before state mutation or actuation
  authorization.
- Watchdog and cycle-budget policies are enforced with deterministic fallback
  behavior.
- Fault handling always prefers bounded degradation over optimistic
  continuation.

Compliance is proven by inspectability artifacts, test traces, and reproducible
builds. A design optimised for throughput at the expense of explainability is
non-compliant.

================================================================================
END OF SPECIFICATION
================================================================================
