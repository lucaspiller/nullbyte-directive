# Nullbyte One Design Brief

This document records the high-level design decisions for `Nullbyte One`, the
standard authority CPU used by Nullbyte-compliant systems. The in-game technical
spec lives at `docs/in-game/technical/compute/nullbyte-one-core.txt`.

## Product Intent

`Nullbyte One` is a deterministic 16-bit compute core designed for:

- Human inspection and certification.
- Physically enforced subsystem separation.
- Large-scale MMO simulation where many cores continue running while players are
  offline.

## Core Narrative Requirements

- The ISA is simple enough for trained human teams to audit end-to-end.
- Independent teams from `SOA`, `BIC`, and `HLS` each build compliant
  implementations.
- Three implementations run in parallel in production hardware.
- An `ECP` voting module gates all authority-path write operations.
- To users it behaves like one CPU, while internally it is triple-executed and
  vote-checked.

## MMO Runtime Requirement

The game server simulates only one reference core per authority module, not all
three hardware cores and the voter internals.

Simulation model requirements:

- Deterministic instruction timing and deterministic fault behavior.
- No nondeterministic hardware effects (cache misses, speculative execution,
  hidden microcode paths).
- Bounded per-tick work so thousands of modules can run concurrently on modern
  CPUs.
- Explicit cycle budgets and overrun faults instead of variable-time execution.

## Architecture Decisions

### Word size and address space

- 16-bit words, 16-bit addressing, 64 KiB total address space.
- The address space is a constitutional limit on complexity, not a hardware
  accident. Small enough to audit the entire memory map. "If it doesn't fit, it
  doesn't ship."

### Byte order

- Big-endian. Most significant word at lower address.
- Matches the bit-field convention in the spec (high bits first) and makes
  memory dumps human-readable during inspection.

### Memory map

- Fixed regions: ROM (0x0000..0x3FFF), RAM (0x4000..0xDFFF), MMIO
  (0xE000..0xEFFF), diagnostics (0xF000..0xF0FF), reserved (0xF100..0xFFFF).
- No runtime remapping. What you see at boot is what you get.
- Self-modifying code is ISA-legal (execute from RAM) but deployment
  certification may restrict it.

### No floating-point unit

- Fixed-point helpers (MUL, MULH, DIV, MOD, QADD, QSUB, SCV) replace an FPU.
- Rationale: FP introduces rounding modes, NaNs, and denormals that are hard to
  certify and could cause voter disagreements between the three independent
  implementations.
- Fixed-point is deterministic and auditable. Slower and more manual, but safe.

### Instruction set

- 7-bit opcode: 4-bit OP (primary class) + 3-bit SUB (instruction within class).
- 35 instructions total across 11 opcode classes (0x0..0xA). OP 0xB..0xF
  reserved.
- Addressing modes encoded in a 3-bit AM field: register direct, register
  indirect, reg+disp8, absolute, immediate, PC-relative, plus two reserved
  (fault on use).

### Cycle costs

- Every instruction has a fixed cycle cost based on its form, never on its
  operands.
- ALU and data movement: 1 cycle.
- MUL/MULH: 2 cycles. DIV/MOD: 3 cycles. Reflects real hardware complexity and
  creates meaningful cost tradeoffs in a 640-cycle budget.
- MMIO: 4 cycles (IN, OUT, BSET, BCLR, BTEST).
- Branches: 1 cycle not-taken, 2 taken.
- Dispatch entry: 5 cycles. ERET return: 4 cycles.

### Tick budget and watchdog

- Ship master clock drives a 100 Hz tick signal.
- Each tick, TICK register resets to 0. Instructions add their cost after
  commit.
- If TICK >= 640 after any commit, the core raises a budget fault.
- The budget is a ceiling, not a quota. Code runs until it halts/waits; the
  budget catches runaway execution.
- Budget fault behavior: core halts for remainder of tick, resumes at VEC_FAULT
  next tick with a fresh budget. If the handler also overruns, core halts
  permanently until external reset.
- The 640-cycle budget is intentionally conservative to allow large population
  simulation while keeping the in-game Flight Core resource-constrained.

### Boot sequence

- PC starts at 0x0000 in ROM. No hidden BIOS or opaque startup.
- ROM contents are defined by the authority profile, not the core spec.
- Typically a small auditable boot program that initializes MMIO, loads from
  external storage into RAM, validates, and jumps to entry point.
- The storage interface is an MMIO device; no dedicated boot bus.

### Dispatch model

- Three vectors in low ROM: VEC_TRAP (0x0008), VEC_EVENT (0x000A), VEC_FAULT
  (0x000C).
- Uniform entry sequence for all three: latch CAUSE, set R0, push
  PC/FLAGS/CAUSE, disable events, jump to vector.
- ERET returns from handlers atomically.
- Budget faults bypass normal dispatch (see tick budget above).

### Event queue

- 4-entry FIFO of 8-bit event ids.
- Deliberately tiny: deep queues cause unpredictable processing spikes.
- Checked at instruction boundaries only when FLAGS.I == 1.
- Overflow faults. If 4 isn't enough, simplify the event sources.

### Calling convention

- R0..R3 arguments, R0/R1 return values, R4/R5 callee-saved, R7 scratch.
- Stack grows downward from 0xE000.
- Not hardware-enforced, but required for authority-path certification.

## Performance Envelope (Simulation First)

Canonical simulation profile (server-side only, not in-game):

- Core clock: `64 kHz` nominal virtual clock.
- Tick rate: `100 Hz`.
- Cycle budget: `640 cycles/tick/core`.

The in-game spec omits the clock speed (it's a server tuning parameter). Players
see the tick rate and cycle budget.

## Compliance and Fabrication Principle

- Compliance is proven by inspectability artifacts, test traces, and
  reproducible builds.
- Designs optimized only for raw throughput are non-compliant if they reduce
  explainability.
- Simplicity is a safety requirement, not a temporary implementation shortcut.
