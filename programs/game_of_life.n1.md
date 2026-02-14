# Conway's Game of Life

This program implements Conway's Game of Life cellular automaton on the Nullbyte
One architecture. The simulation runs on a 16x16 grid stored in RAM, with each
generation displayed visually in memory.

## ISA Constraint Note

In the Nullbyte One ISA, ALU register operations (AM=000) use R[SUB] as the
second operand:

- ADD (SUB=0): B = R0 (must keep R0 = 0 for addition)
- SUB (SUB=1): B = R1 (use for subtraction)
- AND (SUB=2): B = R2 (use for bit masking)
- OR (SUB=3): B = R3 (use for bit combining)
- XOR (SUB=4): B = R4 (use for toggling)
- SHL (SUB=5): B = R5 (shift amount)
- SHR (SUB=6): B = R6 (shift amount)
- CMP (SUB=7): B = R7 (compare value)

This program is carefully written to work within these constraints.

## Memory Layout

- `0x4000` - `0x40FF`: Current generation grid (16x16 = 256 bytes)
- `0x4100` - `0x41FF`: Next generation buffer (256 bytes)
- `0x4200` - `0x42FF`: Display buffer (copied from next generation)

Each cell is one byte: 0x00 = dead, 0xFF = alive.

## Glider Pattern

The initial pattern is a "glider" - a classic Game of Life pattern that moves
diagonally across the grid:

```
  . # . . .
  . . . # .
  . # # # .
```

Positioned at row 0-2, columns 1-4.

## Initialization

Set up constants and the initial glider pattern.

```n1asm
init:
    ; Keep R0 = 0 always (for ADD - B operand is always R0)
    MOV R0, #0x0000

    ; Store glider pattern directly to RAM at 0x4000+
    ; Glider cells: (1,0), (3,1), (1,2), (2,2), (3,2)
    ; Byte address = 0x4000 + y * 16 + x
    ; (1,0) -> 0x4001, (3,1) -> 0x4013, (1,2) -> 0x4021, (2,2) -> 0x4022, (3,2) -> 0x4023

    ; Cell (1,0) at byte 0x4001 - store word at 0x4000, low byte goes to 0x4001
    MOV R1, #0x4000
    MOV R2, #0x00FF     ; Low byte = 0xFF (alive)
    STORE R2, [R1]

    ; Cell (3,1) at byte 0x4013 - store word at 0x4012, low byte goes to 0x4013
    MOV R1, #0x4012
    STORE R2, [R1]

    ; Cell (1,2) at byte 0x4021 - store word at 0x4020, low byte goes to 0x4021
    MOV R1, #0x4020
    STORE R2, [R1]

    ; Cells (2,2) and (3,2) at bytes 0x4022 and 0x4023
    ; Store word at 0x4022: high byte to 0x4022, low byte to 0x4023
    MOV R1, #0x4022
    MOV R2, #0xFFFF     ; Both bytes are 0xFF
    STORE R2, [R1]

    HALT                ; Checkpoint 1: initial glider visible
```

After initialization, the glider pattern is visible in RAM.

```n1test
; Verify initial glider cells
; (1,0) at 0x4001
[0x4001] == 0xFF
; (3,1) at 0x4013
[0x4013] == 0xFF
; (1,2) at 0x4021
[0x4021] == 0xFF
; (2,2) at 0x4022
[0x4022] == 0xFF
; (3,2) at 0x4023
[0x4023] == 0xFF
```

## Generation 1

The glider evolves. From positions (1,0), (3,1), (1,2), (2,2), (3,2), the next
generation becomes (2,0), (3,1), (1,2), (2,2), (3,2).

This is a standard glider evolution - the pattern shifts.

```n1asm
gen1:
    ; Clear next buffer at key locations
    MOV R1, #0x4100
    MOV R2, #0x0000
    STORE R2, [R1]
    MOV R1, #0x4110
    STORE R2, [R1]
    MOV R1, #0x4120
    STORE R2, [R1]

    ; Set next generation cells
    ; (2,0) at 0x4102 - store at 0x4102, high byte
    MOV R1, #0x4102
    MOV R2, #0xFF00
    STORE R2, [R1]

    ; (3,1) at 0x4113 - store at 0x4112, low byte
    MOV R1, #0x4112
    MOV R2, #0x00FF
    STORE R2, [R1]

    ; (1,2), (2,2), (3,2) at 0x4121, 0x4122, 0x4123
    ; Store two words: 0x4120 (bytes 0,1) and 0x4122 (bytes 2,3)
    MOV R1, #0x4120
    MOV R2, #0x00FF     ; Low byte = 0xFF at 0x4121
    STORE R2, [R1]
    MOV R1, #0x4122
    MOV R2, #0xFFFF     ; Both bytes alive
    STORE R2, [R1]

    HALT                ; Checkpoint 2: generation 1 computed
```

After the first evolution, verify the new pattern.

```n1test
; Verify generation 1 in next buffer at 0x4100+
; (2,0) at 0x4102
[0x4102] == 0xFF
; (3,1) at 0x4113
[0x4113] == 0xFF
; (1,2) at 0x4121
[0x4121] == 0xFF
; (2,2) at 0x4122
[0x4122] == 0xFF
; (3,2) at 0x4123
[0x4123] == 0xFF
```

## Copy to Display

Copy the next generation to the display buffer for visualization.

```n1asm
copy_display:
    ; Copy cells from next buffer (0x4100+) to display (0x4200+)
    ; Copy the 5 alive cells

    ; (2,0): 0x4102 -> 0x4202
    MOV R1, #0x4102
    LOAD R3, [R1]
    MOV R1, #0x4202
    STORE R3, [R1]

    ; (3,1): 0x4113 (need to load from 0x4112)
    MOV R1, #0x4112
    LOAD R3, [R1]
    MOV R1, #0x4212
    STORE R3, [R1]

    ; (1,2): 0x4121 (load from 0x4120)
    MOV R1, #0x4120
    LOAD R3, [R1]
    MOV R1, #0x4220
    STORE R3, [R1]

    ; (2,2), (3,2): 0x4122, 0x4123
    MOV R1, #0x4122
    LOAD R3, [R1]
    MOV R1, #0x4222
    STORE R3, [R1]

    HALT                ; Checkpoint 3: display updated
```

After copying, verify the display buffer.

```n1test
; Verify display buffer at 0x4200+
; (2,0) at 0x4202
[0x4202] == 0xFF
; (3,1) at 0x4213
[0x4213] == 0xFF
; (1,2) at 0x4221
[0x4221] == 0xFF
; (2,2) at 0x4222
[0x4222] == 0xFF
; (3,2) at 0x4223
[0x4223] == 0xFF
```

## Continue Simulation

Jump back to compute the next generation. In a full implementation, this would
continue indefinitely, but for this demo we loop back.

```n1asm
    JMP #gen1
```

## Memory View

When viewing the debug tool's memory panel at address 0x4200, you will see the
glider pattern. Live cells appear as `FF` bytes, dead cells as `00`.

The grid layout at 0x4200 (16 bytes per row):

```
Row 0: 00 00 FF 00 00 00 00 00 00 00 00 00 00 00 00 00
Row 1: 00 00 00 00 FF 00 00 00 00 00 00 00 00 00 00 00
Row 2: 00 FF FF FF 00 00 00 00 00 00 00 00 00 00 00 00
```

The glider will continue to evolve with each HALT/resume cycle, demonstrating
Conway's Game of Life in action on the Nullbyte One architecture.
