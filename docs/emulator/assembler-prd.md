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
2. Relocatable object files, separate compilation, or a linker. (The assembler
   supports `.include` for multi-file programs, but all files are merged into a
   single assembly unit -- there is no link step.)
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
- Include directive: `.include "path"` to splice another source file's assembly
  into the current file, forming a single assembly unit.
- Error reporting with file path, line number, and column (with include-chain
  traces for errors in included files).
- Flat binary output (raw bytes, big-endian, loadable by debug tool).
- Inline test format: `n1test` fenced code blocks containing register and memory
  assertions, executed by the assembler's built-in test runner using
  `emulator-core`.
- Test CLI mode: `nullbyte-asm test <input>` assembles the program and runs all
  inline `n1test` blocks against it.

### Out of Scope

- Relocatable object format or linker.
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

### Include Directive

The `.include` directive splices another source file into the current assembly
stream. It is processed before pass 1 -- the assembler expands all includes
recursively, then assembles the merged source as a single unit.

```
.include "path/to/file.n1.md"
```

Rules:

- Paths are resolved relative to the directory of the file containing the
  `.include` directive.
- Included files may themselves contain `.include` directives (recursive).
- Circular includes are detected and reported as errors.
- After expansion, labels and symbols from included files are visible to all
  subsequent code, exactly as if the included source had been written inline.
- Error messages for lines originating in an included file show an include
  chain: `math.n1.md:12 (included from main.n1.md:3)`.
- `.include` may appear inside `n1asm` fenced code blocks (literate format) or
  anywhere in plain assembly files.
- The included file is parsed according to its own extension: `.n1.md` files
  have their `n1asm` code blocks extracted; `.n1` files are treated as raw
  assembly.

### Inline Test Format (`n1test` blocks)

The assembler supports inline tests using fenced code blocks tagged with the
`n1test` language identifier. This turns literate assembly files into
self-testing notebooks -- prose explains the program, `n1asm` blocks define it,
and `n1test` blocks verify it.

Test blocks are only processed when the assembler is invoked in test mode
(`nullbyte-asm test`). In normal assembly mode they are ignored, just like
prose.

Only `.n1.md` files support inline tests (the Markdown structure is needed to
distinguish `n1test` blocks from assembly). Plain `.n1` files can be included
into a literate test file via `.include`.

#### Execution Model

Test execution is HALT-driven. The assembler loads the fully assembled binary
into `emulator-core` and steps the CPU forward. Each `n1test` block corresponds
to the next HALT instruction (or program termination) the CPU reaches. The test
runner checks the block's assertions against machine state at that point, then
resumes execution toward the next HALT for the next test block.

Concretely:

1. Assemble all `n1asm` blocks into a flat binary (same as normal assembly).
2. Load the binary into an `emulator-core` instance.
3. For each `n1test` block in document order: a. Execute until the CPU halts
   (HALT instruction or fault). b. Evaluate all assertions in the block against
   current machine state. c. If any assertion fails, report the failure with
   source location and expected vs. actual values. d. Resume execution (un-halt
   the CPU) to proceed toward the next test block.
4. After the last `n1test` block, report a summary: passed, failed, total.

If the CPU faults before reaching a HALT, the current test block fails with a
fault diagnostic.

#### Assertion Syntax

Each line in an `n1test` block is either an assertion or a comment. Comments use
`;` to end of line, same as assembly.

Assertions take two forms:

| Form               | Meaning                                       |
| ------------------ | --------------------------------------------- |
| `R0 == 0x4000`     | Register value equals expected value.         |
| `[0x4000] == 0xFF` | Memory byte at address equals expected value. |

Register names are `R0`–`R7` and `PC`. Values use the same literal syntax as
assembly operands (decimal, `0x` hex, `0b` binary). Memory assertions use
bracket syntax with an address literal.

The following comparisons are supported `==` and `!=`. No other operators are
supported in v0.1.

#### Example

A complete literate test file:

````markdown
# Blinker Test

## Initialization

```n1asm
init:
    MOV R1, #0x4000
    MOV R2, #0x00FF
    MOV R3, #0x0000
    HALT
```

After initialization, registers hold the setup values and RAM is untouched.

```n1test
R1 == 0x4000
R2 == 0x00FF
R3 == 0x0000
```

## First Toggle

```n1asm
main:
    XOR R3, R3, R2
    STORE R3, [R1]
    HALT
```

After the first XOR, R3 flips to `0xFF` and that value is written to RAM.

```n1test
R3 == 0x00FF
[0x4000] == 0xFF
```

## Second Toggle

```n1asm
    JMP #main
```

Execution jumps back to `main`, toggles again, stores, and halts.

```n1test
R3 == 0x0000
[0x4000] == 0x00
```
````

In this example, the three `n1test` blocks correspond to the three HALTs the CPU
encounters during execution. The test runner steps to each HALT, checks
assertions, resumes, and reports results.

#### Test Blocks and Includes

Test blocks in included files are collected along with the including file's test
blocks. All test blocks across all files are ordered by their position in the
expanded assembly stream and executed sequentially.

## Shared Infrastructure with `emulator-core`

The assembler depends on `emulator-core` for:

1. **`OPCODE_ENCODING_TABLE`**: The canonical `(OP, SUB, OpcodeEncoding)` table
   is the single source of truth for mnemonic-to-opcode mapping. The assembler
   builds its mnemonic lookup from this table at compile time.

2. **`OpcodeEncoding` enum**: Used to match parsed mnemonic strings to their
   canonical encoding variants.

3. **Memory map constants** (`ROM_START`, `ROM_END`, `RAM_START`, etc.): Used
   for address validation and warnings (e.g., code placed outside ROM).

4. **CPU state and execution engine**: The inline test runner loads assembled
   binaries into an `emulator-core` CPU instance, steps execution to HALT
   boundaries, and inspects register and memory state. This requires the
   decoder, execute pipeline, and state model.

If `emulator-core` adds a new opcode to the encoding table, the assembler should
be able to support it by adding the corresponding mnemonic-to-encoding mapping
without duplicating the bit-level encoding logic.

The assembler takes a full dependency on `emulator-core` with no feature gating.
The core runtime is small enough that including it in the assembler binary adds
negligible size and no architectural burden.

## CLI Interface

### Assemble

```
nullbyte-asm build <input> [-o <output>]

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

### Test

```
nullbyte-asm test <input>

Arguments:
  <input>     Source file (.n1 or .n1.md) containing n1test blocks
```

The test command assembles the input, loads the binary into `emulator-core`, and
runs all `n1test` blocks in document order. Each test block's assertions are
checked at the corresponding HALT boundary.

Output: one line per test block (pass/fail with source location), followed by a
summary. Assertion failures show expected vs. actual values.

Exit codes:

- `0`: all tests passed.
- `1`: one or more tests failed or assembly failed.

## Assembly Pipeline

### Pass 0: Include Expansion

1. If input is `.n1.md`, extract fenced code blocks tagged `n1asm` and `n1test`.
2. For each `.include` directive encountered, resolve the path relative to the
   current file, load the target, and recursively expand it.
3. Detect circular includes (maintain a visited-file set) and report an error if
   found.
4. The result is a flat, ordered sequence of assembly lines and test blocks,
   each annotated with its originating file and line number for error reporting.
5. `n1test` blocks are collected separately and associated with their position
   in the assembly stream (used by the test runner, ignored by the assembler).

### Pass 1: Parse and Build Symbol Table

1. Walk the expanded assembly lines from pass 0.
2. Parse each line: recognize labels, instructions, directives, comments.
3. Track the current address (position counter).
4. Record label addresses in the symbol table.
5. Compute instruction sizes (1 word or 2 words depending on addressing mode).
6. Report syntax errors with source location (including include chain).

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

Assembly errors:

- Unknown mnemonic.
- Invalid register name.
- Duplicate label definition.
- Undefined label reference.
- Immediate value out of range.
- Displacement out of signed 8-bit range (AM 010).
- Instruction placed outside ROM region (warning, not error).
- Malformed addressing mode syntax.

Include errors:

- Include file not found.
- Circular include detected (with chain trace).

Test errors:

- Assertion failed: expected vs. actual value with source location.
- CPU faulted before reaching expected HALT.
- More `n1test` blocks than HALT instructions reached during execution.
- Malformed assertion syntax.

Each error includes: file path, line number (in source, not extracted), column
(where feasible), and a human-readable description. Errors in included files
show the full include chain.

## Test Strategy

### Unit Tests (Rust `#[test]`)

- Mnemonic-to-opcode resolution against `OPCODE_ENCODING_TABLE`.
- Instruction encoding for each addressing mode.
- Extension word generation for AM 010, 011, 100, 101.
- Label resolution and PC-relative offset computation.
- Literate format code block extraction (both `n1asm` and `n1test`).
- Data directive emission.
- Include expansion: single-level, recursive, circular detection.
- Error cases: unknown mnemonic, duplicate label, undefined label, range errors.

### Integration Tests (Rust `#[test]`)

- Assemble a known source file and compare output bytes against expected binary.
- Round-trip: assemble a program, load it into `emulator-core`, step through it,
  and verify expected register/memory state.

### Inline Tests (`n1test` blocks)

The inline test format is itself part of the test strategy. Reference test
programs (like the Blinker below) are written as literate `.n1.md` files with
`n1test` blocks. Running `nullbyte-asm test` on them validates both the
assembler's output and the emulator's execution in a single step.

CI should run `nullbyte-asm test` on all `.n1.md` files in a designated test
programs directory (e.g., `tests/programs/`) alongside the Rust unit and
integration tests.

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

1. M1: Crate scaffold, CLI skeleton (`build` and `test` subcommands), literate
   format parser (extract `n1asm` and `n1test` blocks), mnemonic table from
   `emulator-core`.
2. M2: Two-pass assembler for core instruction set (all 41 opcodes, all
   addressing modes, labels, data directives). `.include` directive with
   recursive expansion and cycle detection.
3. M3: Inline test runner -- `n1test` block parsing, assertion evaluation,
   HALT-driven execution via `emulator-core`, pass/fail reporting.
4. M4: Error reporting, edge cases, unit and integration test suite.
5. M5: Reference test programs (Blinker and others) written as literate `.n1.md`
   files with inline tests, validated end-to-end.

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
    `n1asm` / `n1test` tags); no reliance on extended Markdown features.
- Inline test execution depends on `emulator-core` determinism.
  - Mitigation: `emulator-core` is already designed for deterministic execution;
    the assembler test runner uses the same step semantics as the conformance
    test suite.
- Deep include chains could produce confusing error locations.
  - Mitigation: include-chain traces in all error messages; circular include
    detection prevents infinite recursion.

## Dependencies

- `emulator-core` (workspace crate, full dependency): encoding table, memory map
  constants, CPU state model, decoder, and execution engine.
- Standard Rust toolchain (same as workspace).
- No external crate dependencies required for `v0.1`; standard library I/O and
  string handling should suffice.
