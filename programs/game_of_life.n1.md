# Conway's Game of Life

Full algorithmic implementation of Conway's Game of Life on the Nullbyte One
architecture. A 16x16 toroidal grid evolves each generation using the standard
birth/survival rules, verified at each HALT checkpoint.

## ISA Constraint Note

In the Nullbyte One ISA, ALU register operations (AM=000) use R[SUB] as the
second operand. This program uses immediate mode (AM=100) for most ALU
operations to bypass this constraint. The one exception is `ADD Rd, Ra, R0`
which is used to add register R0 (populated with the needed value) to Ra.

## Memory Layout (word-aligned cells)

Each cell occupies one 16-bit word (0x0000=dead, 0x0001=alive). This avoids
byte-extraction logic when loading cells with LOAD.

- `0x4000-0x41FF`: Current generation (16x16 x 2 bytes = 512 bytes)
- `0x4200-0x43FF`: Next generation buffer (512 bytes)
- `0x4400-0x45FF`: Display buffer (512 bytes)

Cell address: `base + y * 32 + x * 2`

## Initialization

Clear all buffers and write the standard glider pattern.

Glider cells: (2,0), (3,1), (1,2), (2,2), (3,2)

```
  . . # . .
  . . . # .
  . # # # .
```

```n1asm
init:
    ; Clear all three buffers (0x4000-0x45FF = 768 words)
    MOV R1, #0x4000
    MOV R2, #0x0000
clear_all:
    STORE R2, [R1]
    ADD R1, R1, #0x0002
    CMP R0, R1, #0x4600
    BLT #clear_all

    ; Write glider pattern (0x0001 = alive)
    ; Cell (x,y) addr = 0x4000 + y*32 + x*2
    MOV R2, #0x0001

    ; (2,0): 0x4000 + 0 + 4 = 0x4004
    MOV R1, #0x4004
    STORE R2, [R1]

    ; (3,1): 0x4000 + 32 + 6 = 0x4026
    MOV R1, #0x4026
    STORE R2, [R1]

    ; (1,2): 0x4000 + 64 + 2 = 0x4042
    MOV R1, #0x4042
    STORE R2, [R1]

    ; (2,2): 0x4000 + 64 + 4 = 0x4044
    MOV R1, #0x4044
    STORE R2, [R1]

    ; (3,2): 0x4000 + 64 + 6 = 0x4046
    MOV R1, #0x4046
    STORE R2, [R1]

    HALT
```

Verify initial glider. Each cell is a 16-bit word stored big-endian, so
the alive value 0x0001 has 0x01 in the low (odd) byte.

```n1test
; Alive cells
[0x4005] == 0x01
[0x4027] == 0x01
[0x4043] == 0x01
[0x4045] == 0x01
[0x4047] == 0x01
; Dead cells (spot check)
[0x4001] == 0x00
[0x4003] == 0x00
```

## Generation Loop

For every cell in the 16x16 grid, count live neighbors (toroidal wrapping),
apply the birth/survival rules, and write the result to the next-generation
buffer. Then copy next-gen to current-gen and display.

Register plan inside the loop body:

- R0: scratch (also serves as ADD B-operand via AM=000)
- R1: scratch / address
- R2: cell value to write
- R3: neighbor count accumulator
- R4: scratch / loaded cell value
- R5: Y loop counter (outer)
- R6: X loop counter (inner)
- R7: unused (CMP uses immediate mode)

```n1asm
gen_loop:
    MOV R5, #0x0000
y_loop:
    MOV R6, #0x0000
x_loop:
    MOV R3, #0x0000

    ; --- Count 8 neighbors ---

    ; (-1, -1)
    ADD R0, R6, #0xFFFF
    AND R0, R0, #0x000F
    ADD R1, R5, #0xFFFF
    AND R1, R1, #0x000F
    CALL #load_and_count

    ; (0, -1)
    MOV R0, R6
    ADD R1, R5, #0xFFFF
    AND R1, R1, #0x000F
    CALL #load_and_count

    ; (1, -1)
    ADD R0, R6, #0x0001
    AND R0, R0, #0x000F
    ADD R1, R5, #0xFFFF
    AND R1, R1, #0x000F
    CALL #load_and_count

    ; (-1, 0)
    ADD R0, R6, #0xFFFF
    AND R0, R0, #0x000F
    MOV R1, R5
    CALL #load_and_count

    ; (1, 0)
    ADD R0, R6, #0x0001
    AND R0, R0, #0x000F
    MOV R1, R5
    CALL #load_and_count

    ; (-1, 1)
    ADD R0, R6, #0xFFFF
    AND R0, R0, #0x000F
    ADD R1, R5, #0x0001
    AND R1, R1, #0x000F
    CALL #load_and_count

    ; (0, 1)
    MOV R0, R6
    ADD R1, R5, #0x0001
    AND R1, R1, #0x000F
    CALL #load_and_count

    ; (1, 1)
    ADD R0, R6, #0x0001
    AND R0, R0, #0x000F
    ADD R1, R5, #0x0001
    AND R1, R1, #0x000F
    CALL #load_and_count

    ; --- Load current cell ---
    MUL R1, R5, #0x0020
    SHL R0, R6, #0x0001
    ADD R1, R1, R0
    ADD R1, R1, #0x4000
    LOAD R4, [R1]

    ; --- Apply rules ---
    ; count==3 -> alive (birth or survival)
    ; count==2 and alive -> alive (survival)
    ; otherwise -> dead
    CMP R0, R3, #0x0003
    BEQ #cell_alive
    CMP R0, R3, #0x0002
    BNE #cell_dead
    CMP R0, R4, #0x0001
    BEQ #cell_alive

cell_dead:
    MOV R2, #0x0000
    JMP #write_next

cell_alive:
    MOV R2, #0x0001

write_next:
    ; Write to next-gen buffer at 0x4200
    MUL R1, R5, #0x0020
    SHL R0, R6, #0x0001
    ADD R1, R1, R0
    ADD R1, R1, #0x4200
    STORE R2, [R1]

    ; --- Advance inner loop ---
    ADD R6, R6, #0x0001
    CMP R0, R6, #0x0010
    BLT #x_loop

    ; --- Advance outer loop ---
    ADD R5, R5, #0x0001
    CMP R0, R5, #0x0010
    BLT #y_loop

    ; --- Copy next-gen to current-gen and display ---
    MOV R5, #0x4200
    MOV R6, #0x4000
    MOV R4, #0x4400
copy_loop:
    LOAD R0, [R5]
    STORE R0, [R6]
    STORE R0, [R4]
    ADD R5, R5, #0x0002
    ADD R6, R6, #0x0002
    ADD R4, R4, #0x0002
    CMP R0, R5, #0x4400
    BLT #copy_loop

    HALT
    JMP #gen_loop
```

## Subroutines

```n1asm
; load_and_count: Load cell at (R0=x, R1=y) from current gen, add to R3.
; Clobbers R0, R1, R4. Preserves R3 (accumulates), R5, R6.
load_and_count:
    MUL R1, R1, #0x0020
    SHL R0, R0, #0x0001
    ADD R1, R1, R0
    ADD R1, R1, #0x4000
    LOAD R4, [R1]
    MOV R0, R4
    ADD R3, R3, R0
    RET
```

## Generation 1 Verification

The glider evolves from (2,0),(3,1),(1,2),(2,2),(3,2) to
(1,1),(3,1),(2,2),(3,2),(2,3).

```n1test
; === Diagnostic: PC should be at 0x0152 (after HALT at 0x0150) ===
PC == 0x0152
; === Diagnostic: display buffer (0x4400+) should have gen 1 data ===
; Cell (3,1) in display: should be alive
[0x4427] == 0x01
; === Current gen (0x4000+) ===
; Gen 1 alive cells
[0x4023] == 0x01
[0x4027] == 0x01
[0x4045] == 0x01
[0x4047] == 0x01
[0x4065] == 0x01
; Cells that died
[0x4005] == 0x00
[0x4043] == 0x00
```

## Generation 2 Verification

Evolves to (3,1),(1,2),(3,2),(2,3),(3,3) -- the glider shifted one step
diagonally.

```n1test
; Gen 2 alive cells
[0x4027] == 0x01
[0x4043] == 0x01
[0x4047] == 0x01
[0x4065] == 0x01
[0x4067] == 0x01
; Cells that died
[0x4023] == 0x00
[0x4045] == 0x00
```
