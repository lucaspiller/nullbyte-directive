# Data Movement Instructions Test

Tests for MOV (OP=0x1), LOAD (OP=0x2), and STORE (OP=0x3).

Note: All tests must be independent - each test sets up its own initial state.

## MOV Immediate Zero

```n1asm
mov_imm_zero:
    MOV R0, #0x0000
    HALT
```

```n1test
R0 == 0x0000
```

## MOV Immediate Max Value

```n1asm
mov_imm_max:
    MOV R1, #0xFFFF
    HALT
```

```n1test
R1 == 0xFFFF
```

## MOV Immediate Pattern

```n1asm
mov_imm_pattern:
    MOV R2, #0x1234
    HALT
```

```n1test
R2 == 0x1234
```

## MOV Register Copy

```n1asm
mov_reg_copy:
    MOV R3, #0x5678
    MOV R4, R3
    HALT
```

```n1test
R3 == 0x5678
R4 == 0x5678
```

## MOV Overwrites Previous Value

```n1asm
mov_overwrite:
    MOV R5, #0xAAAA
    MOV R5, #0x5555
    HALT
```

```n1test
R5 == 0x5555
```

## STORE Then LOAD Round Trip

```n1asm
store_load_roundtrip:
    MOV R0, #0xABCD
    MOV R1, #0x4000
    STORE R0, [R1]
    MOV R0, #0x0000
    LOAD R0, [R1]
    HALT
```

```n1test
R0 == 0xABCD
[0x4000] == 0xAB
[0x4001] == 0xCD
```

## STORE Multiple Values

```n1asm
store_multiple:
    MOV R0, #0x1122
    MOV R1, #0x4010
    STORE R0, [R1]
    MOV R0, #0x3344
    ADD R1, R1, #0x0002
    STORE R0, [R1]
    HALT
```

```n1test
[0x4010] == 0x11
[0x4011] == 0x22
[0x4012] == 0x33
[0x4013] == 0x44
```

## LOAD from Stored Location

First store a value, then in a new test block verify LOAD works.

```n1asm
load_after_store:
    MOV R0, #0xDEAD
    MOV R1, #0x4020
    STORE R0, [R1]
    HALT
```

```n1test
[0x4020] == 0xDE
[0x4021] == 0xAD
```

Now load it back into a different register.

```n1asm
load_verify:
    MOV R1, #0x4020
    MOV R2, #0x0000
    LOAD R2, [R1]
    HALT
```

```n1test
R2 == 0xDEAD
```

## All Registers MOV

```n1asm
all_regs:
    MOV R0, #0x0100
    MOV R1, #0x0200
    MOV R2, #0x0300
    MOV R3, #0x0400
    MOV R4, #0x0500
    MOV R5, #0x0600
    MOV R6, #0x0700
    MOV R7, #0x0800
    HALT
```

```n1test
R0 == 0x0100
R1 == 0x0200
R2 == 0x0300
R3 == 0x0400
R4 == 0x0500
R5 == 0x0600
R6 == 0x0700
R7 == 0x0800
```

## STORE Overwrites Memory

```n1asm
store_overwrite:
    MOV R0, #0x1111
    MOV R1, #0x4030
    STORE R0, [R1]
    MOV R0, #0x2222
    STORE R0, [R1]
    HALT
```

```n1test
[0x4030] == 0x22
[0x4031] == 0x22
```

## MOV Does Not Affect Other Registers

```n1asm
mov_isolated:
    MOV R3, #0xF000
    MOV R4, #0x0F00
    MOV R5, #0x00F0
    MOV R6, #0x000F
    MOV R7, #0x1234
    HALT
```

```n1test
R3 == 0xF000
R4 == 0x0F00
R5 == 0x00F0
R6 == 0x000F
R7 == 0x1234
```
