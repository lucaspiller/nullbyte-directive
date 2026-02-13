NULLBYTE ONE CORE SPECIFICATION Exodus Protocol Compliance Document -- Revision
1.3

================================================================================
PREAMBLE
================================================================================

The Nullbyte One is the standard authority CPU for Exodus-compliant systems. It
exists because the Quiet Burn taught humanity a lesson that cannot be unlearned:
any computer capable of hiding its behavior is capable of hiding a threat.

This core is deliberately simple. It uses a 16-bit architecture with a 64 KiB
address space -- small enough for a trained crew to audit end-to-end. It has no
floating-point unit, no speculative execution, no hidden microcode, and no
runtime code loading. Every instruction costs a fixed number of cycles
regardless of its input data, because three independent implementations (SOA,
BIC, HLS) must produce identical results on every tick, or the ECP voter rejects
the output.

Simplicity is a safety requirement, not a temporary shortcut.

This document is the normative reference for implementers and certification
teams. If something is not specified here, it is not permitted.

================================================================================
AT-A-GLANCE
================================================================================

- 16-bit words, 16-bit address space (0x0000..0xFFFF)
- 8 general-purpose registers (R0..R7)
- Special registers: PC, SP, FLAGS, TICK, CAP, CAUSE, EVP
- Fixed memory map: ROM, RAM, MMIO, diagnostics
- Integer and fixed-point math only (no FPU)
- Fixed cycle cost per instruction (no data-dependent timing)
- Tick budget: 640 cycles/tick at 100 Hz (ship master clock)
- Vectors for TRAP, EVENT, and FAULT dispatch

================================================================================

1. # CORE STATE

The Nullbyte One has fifteen registers. Eight are general-purpose; the rest
serve specific roles in control flow, timing, and fault handling.

Architectural registers:

Name Width Meaning

---

R0..R7 16 General purpose PC 16 Program counter SP 16 Stack pointer FLAGS 16
Status and control flags TICK 16 Cycle counter in current tick CAP 16 Capability
declaration (read-only) CAUSE 16 Trap/event/fault cause (latched at dispatch
entry) EVP 16 Event-pending bitmap (read-only)

FLAGS layout:

Flag Bit Meaning

---

Z 0 Set if result == 0 N 1 Set if result bit 15 == 1 (sign bit) C 2 Carry/borrow
(operation-specific) V 3 Signed overflow I 4 Event enable (1 = events can fire
at boundaries) F 5 Fault-latched (set on fault entry; cleared on reset)

Bits 6..15 read as 0. Writes to them are ignored.

================================================================================ 2)
MEMORY MAP
================================================================================

The entire 64 KiB address space is partitioned into fixed regions. This map
cannot be remapped at runtime. What you see at boot is what you get, forever.
Remapping would allow a compromised core to hide memory contents from
inspection.

Region Range Notes

---

ROM 0x0000..0x3FFF Code and constant tables RAM 0x4000..0xDFFF Writable storage
MMIO 0xE000..0xEFFF Device registers DIAG 0xF000..0xF0FF Diagnostics (read-only)
Reserved 0xF100..0xFFFF Must not be accessed

Execute policy:

Region Fetch Write

---

ROM legal fault RAM legal legal MMIO fault legal\* DIAG fault fault Reserved
fault fault

\*MMIO writes are subject to external voter authorization.

Byte order: big-endian. When a multi-word value is stored in memory, the most
significant word occupies the lower address. This matches the bit-field
convention used throughout this specification (high bits first) and makes memory
dumps readable in natural left-to-right order during inspection.

Self-modifying code is ISA-legal (you can execute from RAM), but deployment
certification may restrict it. If all three cores don't produce the same result,
the voter will catch and raise an execution fault.

================================================================================ 3)
CAPABILITIES (CAP)
================================================================================

The CAP register declares what optional hardware is present. On an
authority-compliant core, all four base capabilities must be active. On
non-authority profiles, these bits are discoverable options.

Bit Name Meaning

---

0 CAP_EVTQ Bounded event queue present 1 CAP_ATOM Atomic MMIO bit ops
(BSET/BCLR/BTEST) 2 CAP_FXH Fixed-point helpers (MULH/QADD/QSUB/SCV) 3 CAP_TRC
Diagnostics trace window present 4..15 Reserved Read as 0 in this revision

Authority compliance requires CAP[0..3] = 1.

================================================================================ 4)
ENCODING
================================================================================

Every instruction fits in one to three 16-bit words. The first word encodes the
operation; additional "extension" words carry addresses, immediates, or
displacement values.

Primary word bitfields:

Bits Field Meaning

---

15..12 OP Primary opcode (16 classes) 11..9 RD Destination register or condition
field 8..6 RA Source register A or mode field 5..3 SUB Source register B or
sub-opcode 2..0 AM Addressing mode

In binary: OOOO DDD AAA SSS MMM

Addressing modes:

AM Name Extra? Effective address

---

000 Register direct no Value is in R[RA] 001 Register indirect no EA = R[RA] 010
Reg + disp8 yes EA = R[RA] + sign_extend(disp8) 011 Absolute yes EA = ext16 100
Immediate yes Value is ext16 101 PC-relative yes EA = PC_next +
sign_extend(ext16) 110 (reserved) -- Illegal encoding fault 111 (reserved) --
Illegal encoding fault

AM=010 has a special rule: the extension word carries an 8-bit signed
displacement in its low byte. The upper byte must be the sign extension (0x00 or
0xFF). If the sign copy is wrong, the core faults. This prevents smuggling data
in unused bits.

Example: LOAD R1, [R3 + 5] encodes as: Word 0: OP=0x2, RD=001, RA=011, SUB=xxx,
AM=010 Word 1: 0x0005 (disp8 = +5, upper byte = 0x00)

================================================================================ 5)
OPCODES
================================================================================

The instruction set is small on purpose. A human team should be able to read the
entire opcode map in one sitting and hold it in memory. The full opcode is 7
bits: 4-bit OP (primary class) and 3-bit SUB (instruction within that class).

Inst OP SUB Cost Description

---

Control (OP=0x0):

NOP 0x0 000 1 Do nothing, advance PC SYNC 0x0 001 1 Barrier: all prior writes
visible HALT 0x0 002 1 Halt for remainder of tick TRAP 0x0 003 1 Software trap
using R[RD] as trap id SWI 0x0 004 1 Software trap using immediate trap id

Data movement (OP=0x1..0x3):

MOV 0x1 -- 1 Copy register or immediate to R[RD] LOAD 0x2 -- 2 Read 16-bit word
from memory into R[RD] STORE 0x3 -- 2 Write R[RD] to memory

Integer ALU (OP=0x4):

ADD 0x4 000 1 R[RD] := A + B, sets Z/N/C/V SUB 0x4 001 1 R[RD] := A - B, sets
Z/N/C/V AND 0x4 002 1 R[RD] := A & B, sets Z/N OR 0x4 003 1 R[RD] := A | B, sets
Z/N XOR 0x4 004 1 R[RD] := A ^ B, sets Z/N SHL 0x4 005 1 R[RD] := A << B[3:0],
sets Z/N/C SHR 0x4 006 1 R[RD] := A >> B[3:0] (arithmetic) CMP 0x4 007 1 Flags
only: computes A - B, discards

Math helpers (OP=0x5):

MUL 0x5 000 2 R[RD] := low 16 bits of A _ B MULH 0x5 001 2 R[RD] := high 16 bits
of signed A _ B DIV 0x5 002 3 R[RD] := A / B (unsigned, truncating) MOD 0x5 003
3 R[RD] := A % B (unsigned remainder) QADD 0x5 004 1 Saturating add: clamp(A +
B) QSUB 0x5 005 1 Saturating sub: clamp(A - B) SCV 0x5 006 1 Saturating
shift/convert (see sec. 7)

MUL and MULH together give you the full 32-bit product of two 16-bit values in
two instructions. MUL handles integer math (array indexing, counters, scaling by
small constants). MULH handles fixed-point scaling (multiply two Q8.8 values,
keep the middle 16 bits).

DIV and MOD cost 3 cycles. More than a multiply, but deterministic. If B is
zero, R[RD] is set to 0 and no fault is raised. This avoids a class of bugs
where unexpected input halts the core.

QADD/QSUB prevent silent overflow in control math. SCV does bounded shift with
saturation. Together the helpers let you do scaled arithmetic without an FPU. It
is slower and more manual, but deterministic and auditable. A floating-point
unit would introduce rounding modes, NaNs, and denormals that are hard to
certify and could cause voter disagreements between the three independent
implementations.

Branch and jump (OP=0x6):

BEQ 0x6 000 1/2 Branch if Z == 1 BNE 0x6 001 1/2 Branch if Z == 0 BLT 0x6 002
1/2 Branch if N != V (signed less) BLE 0x6 003 1/2 Branch if Z==1 or N!=V BGT
0x6 004 1/2 Branch if Z==0 and N==V BGE 0x6 005 1/2 Branch if N == V (signed
greater/eq) JMP 0x6 006 2 Unconditional jump CALL 0x6 007 2 Push return address,
jump (AM=101) RET 0x6 007 2 Pop return address, jump (AM=000)

Branches cost 1 cycle if not taken, 2 if taken.

Stack (OP=0x7):

PUSH 0x7 000 1 SP -= 2, store R[RD] at [SP] POP 0x7 001 1 Load [SP] into R[RD],
SP += 2

MMIO (OP=0x8):

IN 0x8 000 4 Read device register into R[RD] OUT 0x8 001 4 Write R[RD] to device
register

Atomic MMIO (OP=0x9):

BSET 0x9 000 4 Set bit in MMIO word (read-modify-write) BCLR 0x9 001 4 Clear bit
in MMIO word BTEST 0x9 002 4 Test bit in MMIO word, sets Z flag

Event (OP=0xA):

EWAIT 0xA 000 1 Stall until event queue non-empty EGET 0xA 001 1 Dequeue event
id into R[RD] (0 if empty) ERET 0xA 002 4 Return from handler (see sec. 8)

OP 0xB..0xF are reserved. Any unassigned OP or SUB value is an illegal encoding
and triggers a fault.

================================================================================ 6)
OPERAND BINDING
================================================================================

This section defines exactly how each instruction finds its inputs and where it
puts its outputs. Unused register fields must be 000; anything else is an
illegal encoding fault.

Effective address formulas:

AM EA calculation

---

001 EA = R[RA] 010 EA = R[RA] + sign_extend_8(ext16[7:0]) 011 EA = ext16 101 EA
= PC_next + sign_extend_16(ext16)

Binding table:

Instruction Binding

---

MOV AM=000 R[RD] := R[RA] MOV AM=100 R[RD] := ext16 LOAD AM=001/010/011/101
R[RD] := MEM16[EA] STORE AM=001/010/011/101 MEM16[EA] := R[RD] ALU reg AM=000
A=R[RA], B=R[SUB], result -> R[RD] ALU imm AM=100 A=R[RA], B=ext16, result ->
R[RD] Helper reg AM=000 A=R[RA], B=R[SUB], result -> R[RD] Helper imm AM=100
A=R[RA], B=ext16, result -> R[RD] BEQ..BGE AM=101 Tests FLAGS, branch target
from ext16 JMP AM=101 PC := PC_next + sign_extend_16(ext16) CALL AM=101
PUSH(PC_next), then PC := target RET AM=000 PC := POP() PUSH AM=000 SP-=2,
MEM16[SP] := R[RD] POP AM=000 R[RD] := MEM16[SP], SP+=2 IN AM=011 R[RD] :=
MMIO16[ext16] OUT AM=011 MMIO16[ext16] := R[RD] BSET/BCLR/BTEST AM=011 bit_index
= R[RD] & 0x000F EWAIT/ERET AM=000 No register operands EGET AM=000 R[RD] :=
dequeued event id (or 0) TRAP AM=000 trap id = R[RD] & 0xFF SWI AM=100 trap id =
ext16[7:0] (high byte = 0) NOP/SYNC/HALT AM=000 No register operands

================================================================================ 7)
EXECUTION RULES
================================================================================

Every instruction follows the same commit order. This rigid sequencing is what
makes the core deterministic and what lets the three independent implementations
agree on every cycle.

Commit order:

1. Read source operands
2. Compute result and/or effective address
3. Perform memory/MMIO reads
4. Perform memory/MMIO writes
5. Write destination register
6. Update FLAGS
7. Advance PC

Non-control instructions advance PC by 1 + extension words used.

Stack operations: PUSH: SP := SP - 2, then MEM16[SP] := value POP: value :=
MEM16[SP], then SP := SP + 2

If a 16-bit data access lands on an odd address, the core faults.

Branch conditions:

Op Condition Taken Not-taken

---

BEQ Z == 1 2 cy 1 cy BNE Z == 0 2 cy 1 cy BLT N != V 2 cy 1 cy BLE Z==1 or N!=V
2 cy 1 cy BGT Z==0 and N==V 2 cy 1 cy BGE N == V 2 cy 1 cy

Taken target: PC_next + sign_extend_16(ext16).

Example:

CMP R1, R2 ; sets FLAGS based on R1 - R2 BEQ #loop_start ; if equal, branch SUB
R1, R1, R2 ; otherwise, R1 := R1 - R2

FLAGS behavior per instruction:

MOV, LOAD, IN: sets Z/N, clears C and V STORE, OUT: does not touch FLAGS ADD,
SUB, CMP: sets Z/N/C/V AND, OR, XOR: sets Z/N, clears C and V SHL, SHR: sets
Z/N, clears V C set from shifted-out bit (unchanged if S=0) BSET, BCLR: does not
touch FLAGS BTEST: sets Z from tested bit SYNC, HALT: does not touch FLAGS

Math helper details:

MUL: R[RD] := (A _ B) & 0xFFFF (low 16 bits, unsigned) MULH: R[RD] := (signed(A)
_ signed(B)) >> 16 (high 16 bits) DIV: R[RD] := A / B (unsigned, truncated). B=0
sets R[RD]=0. MOD: R[RD] := A % B (unsigned remainder). B=0 sets R[RD]=0. QADD:
R[RD] := clamp(A + B, -32768, 32767) QSUB: R[RD] := clamp(A - B, -32768, 32767)
SCV: Uses ext16: DIR=bit15, S=bits[3:0], bits[14:4]=0 DIR=0: R[RD] := clamp(A <<
S, -32768, 32767) DIR=1: R[RD] := arithmetic_shift_right(A, S)

MUL and MULH do not modify FLAGS. DIV and MOD do not modify FLAGS. QADD, QSUB,
and SCV set Z/N and clear C and V.

Event instructions:

EWAIT: If the event queue is empty, PC stays put (stall). If non-empty, advances
to the next instruction. EGET: If non-empty, R[RD] := event id (zero-extended).
If empty, R[RD] := 0. ERET: Returns from handler. Faults if not in handler
context.

HALT behavior:

HALT retires normally (cost 1), advances PC to PC_next, and then enters halted
state for the remainder of the current tick. Event arrival does not wake a HALTed
core mid-tick. At the next tick boundary, the core resumes execution from the
current PC with a fresh tick budget.

Permanent halt state can still occur through fault escalation paths (section 10
budget recovery failure, section 12 double-fault or invalid fault vector). A
permanently halted core requires external reset.

Example (event polling loop):

EWAIT ; stall until an event arrives EGET R0 ; grab the event id CMP R0, #0x03 ;
is it event 3? BEQ #handle_dock ; yes -- handle docking signal JMP #event_loop ;
no -- wait for the next one

================================================================================ 8)
DISPATCH MODEL (TRAP / EVENT / FAULT)
================================================================================

When something exceptional happens such as a software trap, an external event,
or a fault, the core saves its state and jumps to a handler. The mechanism is
the same for all three; only the vector address and CAUSE encoding differ.

Think of it as the core's panic-and-recover reflex. The handler gets told what
happened (via CAUSE and R0), deals with it, and returns.

Vectors:

Vector Address Used for

---

VEC_TRAP 0x0008 Software traps (TRAP, SWI) VEC_EVENT 0x000A External events from
the event queue VEC_FAULT 0x000C Faults (illegal ops, bad access, etc.)

CAUSE register layout:

Bits Meaning

---

15..12 Class: 1=TRAP, 2=EVENT, 3=FAULT 11..8 Subcode namespace (authority
profile uses 0) 7..0 Trap id, event id, or fault code

Where the low byte comes from:

TRAP: R[RD] & 0xFF SWI: ext16[7:0] EVENT: Dequeued event id FAULT:
Profile-defined fault code

Entry sequence (same for all three classes):

1. Latch CAUSE
2. Set R0 := CAUSE & 0x00FF (handler gets the code fast)
3. PUSH resume PC
4. PUSH pre-entry FLAGS
5. PUSH CAUSE
6. Clear FLAGS.I (disable events during handler)
7. Set PC := handler vector

Return sequence (ERET):

1. POP into CAUSE shadow
2. POP into FLAGS
3. POP into PC
4. Resume execution

Example:

SWI #0x02 ; software interrupt, trap id = 2 ; -- core pushes PC, FLAGS, CAUSE
onto stack -- ; -- jumps to [VEC_TRAP] at 0x0008 --

; ... in the trap handler at 0x0008: CMP R0, #0x02 ; R0 was set to the trap id
BEQ #handle_reboot ; dispatch to the right routine ERET ; return to caller

Dispatch costs:

TRAP/EVENT entry: 5 cycles (after the issuing instruction) FAULT entry: 5 cycles
(+ faulting instruction's base cost) ERET return: 4 cycles

================================================================================ 9)
EVENT QUEUE
================================================================================

The event queue is how the outside world gets the core's attention: a docking
signal, a sensor alert, a timer tick from another subsystem. Events arrive as
8-bit ids and wait in a small FIFO until the core is ready to handle them.

The queue is deliberately tiny. A deep queue would let events pile up and cause
unpredictable processing spikes, breaking the core's timing guarantees. If your
system generates events faster than you handle them, that is a design problem,
not a queue-size problem.

Property Value

---

Structure Fixed-depth FIFO Depth 4 entries Payload 8-bit event id (no additional
data) Sampling Checked at instruction boundaries only Reordering Forbidden

If FLAGS.I == 1 and the queue is non-empty at an instruction boundary, the core
performs an EVENT dispatch.

If an enqueue occurs while the queue is full, the core faults. Four slots is
enough if you handle events promptly. If it isn't enough, simplify your event
sources.

================================================================================ 10)
TIMING
================================================================================

Every instruction has a fixed cycle cost that depends only on its form, never on
its operands. This is non-negotiable: the ECP voter compares outputs from three
independently-built cores on every tick. If one core took a different number of
cycles because of a data-dependent shortcut, the voter would flag a disagreement
and reject the output.

Instruction/form Cycles

---

NOP, SYNC, HALT 1 TRAP issue, SWI issue 1 MOV (reg or imm) 1 LOAD, STORE 2 ADD,
SUB, AND, OR, XOR, SHL, SHR, CMP 1 MUL, MULH 2 DIV, MOD 3 QADD, QSUB, SCV 1
BEQ..BGE (not taken) 1 BEQ..BGE (taken) 2 JMP 2 CALL 2 RET 2 PUSH, POP 1 IN, OUT
4 BSET, BCLR, BTEST 4 EWAIT, EGET 1 TRAP dispatch entry 5 EVENT dispatch entry 5
FAULT dispatch entry 5 (+base) ERET return 4

Tick budget (authority profile):

Tick rate: 100 Hz (one tick every 10 ms) Budget per tick: 640 cycles

The authority profile specifies 640 cycles per tick at 100 ticks per second. The
budget exists because the compute cores share power, thermal, and scheduling
resources with every other system on the ship. It is deliberately conservative.
Enough for responsive control logic, not enough for runaway computation. Other
profiles may define different budgets.

Tick mechanism:

The ship's master clock drives a 100 Hz tick signal to the core. At each tick
boundary:

1. TICK resets to 0
2. The core begins executing from its current PC

Each instruction adds its cycle cost to TICK after commit. If TICK >= 640 after
any instruction commits, the core raises a budget fault.

The budget is a ceiling, not a quota. You do not have to use all 640 cycles.
Typical code does its work and reaches HALT or EWAIT well before the limit. The
budget only fires if code runs away -- an infinite loop, an unexpectedly long
handler chain, or simply too much work for one tick.

Budget fault behavior:

When a budget fault fires, the core does not attempt normal fault dispatch (the
handler would itself cost cycles the core no longer has). Instead:

1. The faulting PC and TICK value are latched into the diagnostics window
2. The core enters halted state for the remainder of the tick
3. On the next tick boundary, the core resumes from the fault vector (VEC_FAULT)
   with a fresh 640-cycle budget
4. The fault handler has one full tick to recover -- log the fault, reset state,
   or request external intervention
5. If the fault handler itself exceeds the budget, the core halts and stays
   halted until an external reset

This gives software one chance to recover. If it cannot recover within a single
tick, the core stops. A halted core is a safe core.

================================================================================ 11)
RESET AND BOOT
================================================================================

On reset, the core initializes to a known state and begins executing at address
0x0000 in ROM. There is no hidden BIOS or opaque startup sequence, the ROM
contents are auditable code like anything else.

The authority profile defines what the ROM contains. Typically this is a small
boot program that initializes MMIO devices, loads a program image from external
storage into RAM, validates it, and jumps to the entry point. If the load or
validation fails, the boot program halts the core. The storage interface is an
MMIO device; the core does not have a dedicated boot bus.

Register Reset value

---

PC 0x0000 SP 0xE000 (top of RAM, grows downward) R0..R7 0x0000 FLAGS 0x0000
(events disabled, no fault latched) TICK 0x0000 EVP 0x0000 CAUSE 0x0000 CAP Set
by hardware (capability bits)

Reset also clears the event queue and any latched fault state.

SP starts at 0xE000 because that is the boundary between RAM and MMIO. The stack
grows downward into RAM. SP must remain even- aligned at all function and
handler boundaries.

================================================================================ 12)
FAULT CONTRACT
================================================================================

The core makes a simple promise: if something is wrong, it will tell you. It
will not silently do the wrong thing.

Fault triggers:

- Illegal instruction encoding
- Memory access to reserved or non-readable region
- MMIO access that violates width or alignment rules
- Instruction fetch from non-executable region
- Event queue overflow
- Unaligned 16-bit data access

When a fault fires, normal instruction retirement stops and control transfers to
the fault handler at [VEC_FAULT].

If no valid fault handler exists (VEC_FAULT points to garbage or a double-fault
occurs during handling), the core halts. A halted core is a safe core. Silence
is better than corruption.

Budget faults are handled differently -- see section 10. The core does not
attempt normal dispatch because the handler would cost cycles the core no longer
has.

Detailed fault taxonomy and priority ordering are defined per deployment
profile.

================================================================================ 13)
MMIO CONTRACT
================================================================================

MMIO is how the core talks to the ship's hardware. Reading sensor latches,
commanding actuators, checking status lines, or blinken the LED. All MMIO lives
in the range 0xE000..0xEFFF.

Rules:

- All access is 16-bit. No byte access. Byte access faults.
- Operations are strongly ordered with respect to other memory and MMIO
  operations. No reordering, no buffering surprises.
- OUT side effects become visible at instruction commit.
- SYNC forces all prior memory/MMIO effects to be visible before the next
  instruction executes.

Write authorization:

MMIO writes in authority assemblies are gated by the ECP voter. If the voter
inhibits a write, it is silently suppressed and the inhibit state is latched for
diagnostics.

The core keeps running, it just didn't get to touch that device. This is the
hardware-enforced boundary between "the core wants to" and "the ship allows it."

Devices must not mutate CPU registers directly. All communication goes through
the MMIO window.

================================================================================ 14)
DIAGNOSTICS WINDOW
================================================================================

The diagnostics window at 0xF000..0xF0FF is a read-only view into the core's
fault history and health counters. It exists because Exodus Protocol mandates
inspectability: you should always be able to ask the core "what went wrong?"
without needing special tools.

Exposed data:

- Last fault code
- Last faulting PC
- Last fault tick index
- Per-class fault counters (saturating 16-bit)
- Executed-instruction counter (saturating 16-bit)

Writing to the diagnostics window triggers a fault. It is read- only for the
same reason a flight recorder should not have an erase button.

================================================================================ 15)
CALLING CONVENTION
================================================================================

The following conventions allow independently-written code modules to call each
other predictably. They are not enforced by hardware, but violating them in
authority-path code will break certification.

Stack model:

- Stack grows downward
- PUSH: SP -= 2, then store
- POP: read, then SP += 2

Register usage:

Role Registers

---

Arguments R0, R1, R2, R3 Return values R0 (primary), R1 (secondary) Caller-saved
R0..R3, R6, R7, condition flags Callee-saved R4, R5 Scratch R7 (assembler
temporary)

Callee must preserve R4, R5, and balanced SP.

Handler convention:

- On entry, R0 = CAUSE low byte
- Handlers preserve R4, R5, and balanced SP
- Handlers may clobber R0..R3, R6, R7, and flags
- Handlers return via ERET (do not manually pop the frame)

Example (simple function):

; add_offset(R0=base, R1=offset) -> R0=result add_offset: ADD R0, R0, R1 ; R0 :=
base + offset RET ; return to caller

; calling it: MOV R0, #0x4000 ; base address MOV R1, #0x0010 ; offset CALL
#add_offset ; R0 now holds 0x4010

================================================================================ 16)
DETERMINISM AND COMPLIANCE
================================================================================

The Nullbyte One exists because the Quiet Burn proved that opaque,
self-modifying systems cannot be trusted with authority over human lives. Every
design choice in this core traces back to one principle: if you cannot inspect
it, you cannot trust it.

Implementations must not introduce architecturally observable divergence. The
following are explicitly forbidden:

- Data-dependent instruction timing
- Speculative execution with visible side effects
- Hidden mutable microcode that changes opcode semantics
- Runtime remapping of memory or MMIO windows
- Unconstrained device-to-CPU side effects

The register file is exactly R0..R7. Expanded register-file profiles fall
outside authority compliance.

Three independent organizations (SOA, BIC, HLS) build their own compliant
implementations from this specification. In production authority assemblies, all
three run in lockstep and an ECP voter gates every write. If any core disagrees,
the write is rejected.

================================================================================ 17)
NOTATION REFERENCE
================================================================================

Shorthand used throughout this document.

Notation Meaning

---

R[x] Value of register x MEM16[a] 16-bit memory word at address a MMIO16[a]
16-bit MMIO word at address a [a] Dereference (width from context) ext16 16-bit
extension word (follows primary word) PC_next PC after consuming current
instruction's words sign_extend_8 Sign-extend 8-bit value to 16 bits
sign_extend_16 Signed interpretation of 16-bit value zero_extend_8 Zero-extend
8-bit value to 16 bits sat_s16(v) Clamp v to signed 16-bit range bit(n,v) Bit n
of value v

================================================================================
END OF SPECIFICATION
================================================================================
