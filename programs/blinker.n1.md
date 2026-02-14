# Blinker: RAM Toggle Demo

This program demonstrates basic memory writes by toggling a byte in RAM between
`0xFF` and `0x00` on each tick. It exercises MOV, STORE, XOR, and HALT
instructions.

## ISA Constraint Note

In the Nullbyte One ISA, ALU register operations (AM=000) use R[SUB] as the
second operand. Since XOR has SUB=4, the XOR instruction always uses R4 as the
second operand. This program is written to work within this constraint.

## Initialization

Set up the RAM target address and the toggle mask in R4 (which XOR uses).

```n1asm
init:
    MOV R1, #0x4000     ; R1 = RAM base address
    MOV R4, #0x00FF     ; R4 = toggle mask (must be R4 for XOR)
    MOV R3, #0x0000     ; R3 = current value (starts at 0)
    HALT                ; stop here for test checkpoint
```

After initialization, registers hold the setup values.

```n1test
R1 == 0x4000
R3 == 0x0000
R4 == 0x00FF
```

## Main Loop

Each tick, XOR the current value with the mask and store it. XOR uses R4 as the
second operand (because XOR has SUB=4). STORE writes a 16-bit word, so 0x00FF
stored at 0x4000 puts 0xFF at address 0x4001 (big-endian).

```n1asm
main:
    XOR R3, R3, R4      ; toggle: R3 = R3 XOR R4 (0x00 -> 0xFF -> 0x00 -> ...)
    STORE R3, [R1]      ; write 16-bit word to RAM at 0x4000 (big-endian)
    HALT                ; wait for next tick
```

After the first XOR, R3 is 0x00FF. STORE writes this as big-endian, so byte at
0x4001 is 0xFF.

```n1test
R3 == 0x00FF
[0x4001] == 0xFF
```

## Loop Back

Jump back to the main loop to continue toggling.

```n1asm
    JMP #main
```

After jumping back, the XOR toggles again (0xFF XOR 0xFF = 0x0000), stores, and
halts. The low byte at 0x4001 is now 0x00.

```n1test
R3 == 0x0000
[0x4001] == 0x00
```

The memory view in the debugger will show address `0x4001` alternating between
`FF` and `00` on each step-tick cycle.
