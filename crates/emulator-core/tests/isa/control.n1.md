# Control Instructions Test

Tests for the control instruction class (OP=0x0): NOP, SYNC, HALT.

## NOP

NOP performs no operation and advances PC.

```n1asm
init:
    MOV R0, #0x1234
    NOP
    HALT
```

R0 should be unchanged after NOP.

```n1test
R0 == 0x1234
```

## SYNC

SYNC provides memory ordering visibility.

```n1asm
sync_test:
    MOV R1, #0xABCD
    SYNC
    HALT
```

```n1test
R1 == 0xABCD
```

## HALT

HALT stops execution for the remainder of the tick.

```n1asm
halt_test:
    MOV R2, #0x0001
    HALT
    MOV R2, #0x0002
    HALT
```

After first HALT, R2 should be 0x0001.

```n1test
R2 == 0x0001
```

Resume execution - second MOV should execute.

```n1test
R2 == 0x0002
```

## Multiple NOPs

Test that multiple NOPs execute in sequence.

```n1asm
multi_nop:
    MOV R3, #0x0000
    NOP
    NOP
    NOP
    MOV R3, #0x0001
    HALT
```

```n1test
R3 == 0x0001
```

## NOP with Different Register States

NOP should not affect any registers.

```n1asm
nop_state:
    MOV R0, #0xFFFF
    MOV R1, #0x0000
    MOV R2, #0x1234
    MOV R3, #0x5678
    NOP
    HALT
```

All registers should be preserved.

```n1test
R0 == 0xFFFF
R1 == 0x0000
R2 == 0x1234
R3 == 0x5678
```

## SYNC Between Operations

SYNC should act as a barrier without affecting registers.

```n1asm
sync_barrier:
    MOV R4, #0x1111
    SYNC
    MOV R5, #0x2222
    SYNC
    HALT
```

```n1test
R4 == 0x1111
R5 == 0x2222
```

## HALT Sequence

Test sequential HALTs with operations between them.

```n1asm
halt_sequence:
    MOV R6, #0x0001
    HALT
    ADD R6, R6, #0x0001
    HALT
    ADD R6, R6, #0x0001
    HALT
```

Step through three HALT checkpoints.

```n1test
R6 == 0x0001
```

```n1test
R6 == 0x0002
```

```n1test
R6 == 0x0003
```
