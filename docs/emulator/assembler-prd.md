# Assembler PRD

## Document Status

- Owner: Tooling
- Last Updated: 2026-02-14
- Status: Draft v0.1

## Context

`assembler` is a command-line assembler for Nullbyte One programs. It reads
literate assembly source files (`*.n1` or `*.n1.md`) and emits flat binary ROM
images that can be loaded directly into the debug tool or any other
`emulator-core` host.

The assembler reuses opcode and encoding definitions from `emulator-core` so
that there is a single source of truth for instruction semantics. If an opcode
exists in the encoding table, the assembler can emit it; if it doesn't, the
assembler rejects it at parse time.

## Problem Statement

We need a way to write Nullbyte One programs that is:

- Human readable and editable without manual bit-level encoding.
- Correct by construction against the ISA encoding table.
- Capable of producing flat ROM binaries loadable by the debug tool and any
  `emulator-core` host.
- Supportive of literate programming, so programs can double as documentation
  and learning material.

Without an assembler, writing test programs for the debug tool requires
hand-encoding 16-bit instruction words and extension words as raw bytes. This is
error-prone, unreadable, and does not scale beyond trivial programs.

## Product Goals

1. Assemble Nullbyte One source into correct flat binary ROM images.
2. Support a literate programming source format where prose and code coexist.
3. Reuse `emulator-core::encoding` tables as the single source of truth for
   valid opcodes and their bit assignments.
4. Provide clear, actionable error messages with source location.
5. Ship as a Rust CLI tool in `crates/assembler/`.

## Non-Goals (For This PRD)

1. Optimizing generated code (instruction scheduling, peephole).
2. Linking multiple object files or library support.
3. Macro preprocessing or conditional assembly.
4. Source-level debug info emission (DWARF or custom format).
5. Interactive or watch-mode assembly.

## Target Users

1. Emulator/core engineers writing test programs and validation fixtures.
2. Tooling engineers validating the debug tool end-to-end.
3. Content/systems engineers authoring ROM programs and learning the ISA.

## Proposed Product Scope (v0.1)

### In Scope

- Crate location: `crates/assembler/`.
- CLI binary: `nullbyte-asm` (via `[[bin]]` in `Cargo.toml`).
- Literate source format: Markdown files where fenced code blocks contain
  assembly and everything else is prose (see Source Format section below).
- Plain assembly format: files containing only assembly source (no Markdown
  wrapper).
- Two-pass assembly: pass 1 builds symbol table, pass 2 emits binary.
- All 41 opcodes from `emulator-core::OPCODE_ENCODING_TABLE`.
- All 6 valid addressing modes (register direct through PC-relative).
- Extension word emission for addressing modes that require them (AM 010, 011,
  100, 101).
- Labels for branch/jump targets and data references.
- Numeric literals: decimal, hexadecimal (`0x`-prefixed), binary
  (`0b`-prefixed).
- Named register operands (`R0`–`R7`).
- Comment syntax: `;` to end of line.
- Data directives: `.word` (16-bit), `.byte` (8-bit), `.ascii`, `.zero`.
- Origin directive: `.org` to set output address.
- Error reporting with file path, line number, and column.
- Flat binary output (raw bytes, big-endian, loadable by debug tool).

### Out of Scope

- Relocatable object format or linker.
- Include/import directives across files.
- Macro system.
- Assembler listing output (`.lst` files).
- Integration with the debug tool UI (assembly happens offline via CLI).

## Source Format

### Literate Assembly (`*.n1.md`)

The assembler supports a literate programming format based on Markdown. Assembly
source lives inside fenced code blocks tagged with the `n1asm` language
identifier. Everything outside code blocks is treated as prose documentation and
ignored by the assembler.

Code blocks are assembled in document order. Labels and symbols defined in
earlier blocks are visible in later blocks, forming a single assembly unit.

Example:

````markdown
# Blinker: RAM Toggle Demo

This program demonstrates basic memory writes by toggling a byte in RAM between
`0xFF` and `0x00` on each tick. It exercises MOV, STORE, LOAD, XOR, and HALT
instructions.

## Initialization

Set up the RAM target address and the toggle mask.

```n1asm
init:
    MOV R1, #0x4000     ; R1 = RAM base address
    MOV R2, #0x00FF     ; R2 = toggle mask (0xFF)
    MOV R3, #0x0000     ; R3 = current value (starts at 0)
```

## Main Loop

Each tick, XOR the current value with the mask and store it. Then halt to wait
for the next tick boundary.

```n1asm
main:
    XOR R3, R3, R2      ; toggle: 0x00 -> 0xFF -> 0x00 -> ...
    STORE R3, [R1]      ; write to RAM at 0x4000
    HALT                ; wait for next tick
    JMP #main           ; resume at top of loop
```

The memory view in the debugger will show address `0x4000` alternating between
`00` and `FF` on each step-tick cycle.
````

### Plain Assembly (`*.n1`)

Files without the `.md` extension are treated as plain assembly. The entire file
content is assembly source. Comments use `;` to end of line. No Markdown parsing
is performed.

```
; Blinker: RAM Toggle Demo
init:
    MOV R1, #0x4000
    MOV R2, #0x00FF
    MOV R3, #0x0000
main:
    XOR R3, R3, R2
    STORE R3, [R1]
    HALT
    JMP #main
```

### Instruction Syntax

Instructions follow this general pattern:

```
[label:] MNEMONIC [RD, ] [RA, ] [RB | #imm | [RA] | [RA + disp]]
```

Addressing modes are expressed through operand syntax:

| Syntax         | Addressing Mode        | AM bits | Extension? |
| -------------- | ---------------------- | ------- | ---------- |
| `RB`           | Register direct        | 000     | No         |
| `[RA]`         | Register indirect      | 001     | No         |
| `[RA + disp8]` | Reg + signed disp8     | 010     | Yes (disp) |
| `#abs16`       | Absolute / Immediate   | 011/100 | Yes (ext)  |
| `#label`       | PC-relative (resolved) | 101     | Yes (ext)  |

The assembler determines the correct addressing mode from the operand form. For
branch and jump instructions, label references are resolved as PC-relative
offsets (AM 101). For MOV and ALU immediate forms, `#value` uses AM 100. For
LOAD/STORE with `#addr`, the assembler uses AM 011 (absolute).

### Data Directives

| Directive      | Description                                |
| -------------- | ------------------------------------------ |
| `.org addr`    | Set the output position counter to `addr`. |
| `.word val`    | Emit a 16-bit value (big-endian).          |
| `.byte val`    | Emit an 8-bit value.                       |
| `.ascii "str"` | Emit ASCII bytes (no null terminator).     |
| `.zero count`  | Emit `count` zero bytes.                   |

## Shared Infrastructure with `emulator-core`

The assembler depends on `emulator-core` for:

1. **`OPCODE_ENCODING_TABLE`**: The canonical `(OP, SUB, OpcodeEncoding)` table
   is the single source of truth for mnemonic-to-opcode mapping. The assembler
   builds its mnemonic lookup from this table at compile time.

2. **`OpcodeEncoding` enum**: Used to match parsed mnemonic strings to their
   canonical encoding variants.

3. **Memory map constants** (`ROM_START`, `ROM_END`, `RAM_START`, etc.): Used
   for address validation and warnings (e.g., code placed outside ROM).

If `emulator-core` adds a new opcode to the encoding table, the assembler should
be able to support it by adding the corresponding mnemonic-to-encoding mapping
without duplicating the bit-level encoding logic.

The assembler should not depend on `emulator-core` features that pull in runtime
execution machinery (decoder, execute pipeline, state model). Only the
`encoding` and `memory::map` modules are needed. If those modules are behind a
feature gate, use it; otherwise, direct dependency on the full crate is
acceptable for `v0.1`.

## CLI Interface

```
nullbyte-asm <input> [-o <output>]

Arguments:
  <input>     Source file (.n1 or .n1.md)

Options:
  -o <output>   Output binary path (default: input stem + .bin)
  --verbose     Print assembly listing to stderr
  --help        Print usage
```

Exit codes:

- `0`: assembly succeeded.
- `1`: assembly failed (errors printed to stderr).

## Assembly Pipeline

### Pass 1: Parse and Build Symbol Table

1. If input is `.n1.md`, extract code blocks tagged `n1asm`.
2. Parse each line: recognize labels, instructions, directives, comments.
3. Track the current address (position counter).
4. Record label addresses in the symbol table.
5. Compute instruction sizes (1 word or 2 words depending on addressing mode).
6. Report syntax errors with source location.

### Pass 2: Emit Binary

1. Walk parsed instructions in order.
2. Resolve label references to addresses.
3. For branch/jump labels, compute PC-relative offset and emit as AM 101.
4. Encode each instruction as a 16-bit primary word using the bit layout from
   the ISA spec: `[OP:4][RD:3][RA:3][SUB:3][AM:3]`.
5. Emit extension words where required (AM 010, 011, 100, 101).
6. Emit data directive bytes.
7. Write the output buffer as a flat binary file.

### Encoding Rules

Primary word layout:

```
Bit 15  14  13  12  11  10   9   8   7   6   5   4   3   2   1   0
  [ OP          ] [ RD         ] [ RA         ] [ SUB        ] [ AM    ]
```

The `OP` and `SUB` values come directly from `OPCODE_ENCODING_TABLE`. The
assembler maps mnemonic names to `OpcodeEncoding` variants and looks up the
corresponding `(OP, SUB)` pair.

Extension word rules by addressing mode:

- AM 000 (register direct): no extension word.
- AM 001 (register indirect): no extension word.
- AM 010 (reg + disp8): extension word with sign-extended displacement. Low byte
  = disp8, high byte = sign copy (0x00 or 0xFF).
- AM 011 (absolute): extension word = 16-bit absolute address.
- AM 100 (immediate): extension word = 16-bit immediate value.
- AM 101 (PC-relative): extension word = signed 16-bit offset from PC_next.

## Error Model

Errors include:

- Unknown mnemonic.
- Invalid register name.
- Duplicate label definition.
- Undefined label reference.
- Immediate value out of range.
- Displacement out of signed 8-bit range (AM 010).
- Instruction placed outside ROM region (warning, not error).
- Malformed addressing mode syntax.

Each error includes: file path, line number (in source, not extracted), column
(where feasible), and a human-readable description.

## Test Strategy

### Unit Tests

- Mnemonic-to-opcode resolution against `OPCODE_ENCODING_TABLE`.
- Instruction encoding for each addressing mode.
- Extension word generation for AM 010, 011, 100, 101.
- Label resolution and PC-relative offset computation.
- Literate format code block extraction.
- Data directive emission.
- Error cases: unknown mnemonic, duplicate label, undefined label, range errors.

### Integration Tests

- Assemble a known source file and compare output bytes against expected binary.
- Round-trip: assemble a program, load it into `emulator-core`, step through it,
  and verify expected register/memory state.
- Assemble the reference test programs (see below) and validate in the debug
  tool.

## Reference Test Program: Blinker

The initial test program is deliberately simple. It exercises:

- `MOV` with immediate addressing (AM 100).
- `XOR` with register operands (AM 000).
- `STORE` with register indirect (AM 001).
- `HALT` for tick boundary.
- `JMP` with PC-relative addressing (AM 101).
- Label definitions and forward references.

The expected behavior is visible in the debug tool's memory view: address
`0x4000` toggles between `0x00` and `0xFF` on each step-tick cycle.

This validates:

1. The assembler produces correct binary output.
2. The debug tool loads and executes the binary correctly.
3. The memory view reflects instruction side effects.
4. Tick boundary behavior (HALT + resume) works as expected.

A more complex test program (such as Conway's Game of Life operating on a RAM
grid and observable through the memory view) is a follow-up milestone after the
assembler and debug tool are validated with simpler programs.

## Milestones

1. M1: Crate scaffold, CLI skeleton, literate format parser, mnemonic table from
   `emulator-core`.
2. M2: Two-pass assembler for core instruction set (all 41 opcodes, all
   addressing modes, labels, data directives).
3. M3: Error reporting, edge cases, unit and integration test suite.
4. M4: Reference test programs assembled and validated in debug tool.

## Risks and Mitigations

- Encoding table drift between `emulator-core` and assembler.
  - Mitigation: single source of truth via crate dependency on
    `emulator-core::OPCODE_ENCODING_TABLE`.
- Addressing mode ambiguity in syntax (e.g., `#value` as absolute vs.
  immediate).
  - Mitigation: context-dependent resolution documented in the syntax table
    above; specific instructions have fixed valid addressing modes.
- Literate format fragility if Markdown flavors differ.
  - Mitigation: only support standard fenced code blocks (triple backtick with
    `n1asm` tag); no reliance on extended Markdown features.

## Dependencies

- `emulator-core` (workspace crate): encoding table, memory map constants.
- Standard Rust toolchain (same as workspace).
- No external crate dependencies required for `v0.1`; standard library I/O and
  string handling should suffice.
