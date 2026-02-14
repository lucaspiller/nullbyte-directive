# ALU Instructions Test

Tests for ALU operations (OP=0x4): ADD, SUB, AND, OR, XOR, SHL, SHR, CMP.

## ADD Immediate

```n1asm
add_imm:
    MOV R0, #0x0010
    ADD R0, R0, #0x0020
    HALT
```

```n1test
R0 == 0x0030
```

## ADD Register

ADD with register mode uses R[SUB] as second operand (SUB=0 means R0).

```n1asm
add_reg:
    MOV R1, #0x0100
    MOV R0, #0x0010
    ADD R2, R1, R0
    HALT
```

```n1test
R2 == 0x0110
```

## ADD Carry

ADD that overows sets carry flag.

```n1asm
add_carry:
    MOV R0, #0xFFFF
    ADD R0, R0, #0x0001
    HALT
```

```n1test
R0 == 0x0000
```

## SUB Immediate

```n1asm
sub_imm:
    MOV R0, #0x0100
    SUB R0, R0, #0x0020
    HALT
```

```n1test
R0 == 0x00E0
```

## SUB Borrow

SUB that underflows.

```n1asm
sub_borrow:
    MOV R0, #0x0000
    SUB R0, R0, #0x0001
    HALT
```

```n1test
R0 == 0xFFFF
```

## SUB Equal

```n1asm
sub_equal:
    MOV R0, #0x1234
    SUB R0, R0, #0x1234
    HALT
```

```n1test
R0 == 0x0000
```

## AND Immediate

```n1asm
and_imm:
    MOV R0, #0xFF00
    AND R0, R0, #0x0F0F
    HALT
```

```n1test
R0 == 0x0F00
```

## AND Zero Result

```n1asm
and_zero:
    MOV R0, #0xFF00
    AND R0, R0, #0x00FF
    HALT
```

```n1test
R0 == 0x0000
```

## OR Immediate

```n1asm
or_imm:
    MOV R0, #0xF000
    OR R0, R0, #0x000F
    HALT
```

```n1test
R0 == 0xF00F
```

## OR Same Value

```n1asm
or_same:
    MOV R0, #0x1234
    OR R0, R0, #0x1234
    HALT
```

```n1test
R0 == 0x1234
```

## XOR Immediate

```n1asm
xor_imm:
    MOV R0, #0xFF00
    XOR R0, R0, #0xFF00
    HALT
```

```n1test
R0 == 0x0000
```

## XOR Toggle Bits

```n1asm
xor_toggle:
    MOV R0, #0x00FF
    XOR R0, R0, #0xFFFF
    HALT
```

```n1test
R0 == 0xFF00
```

## SHL Immediate

```n1asm
shl_imm:
    MOV R0, #0x0001
    SHL R0, R0, #0x0004
    HALT
```

```n1test
R0 == 0x0010
```

## SHL Maximum Shift

```n1asm
shl_max:
    MOV R0, #0x0001
    SHL R0, R0, #0x000F
    HALT
```

```n1test
R0 == 0x8000
```

## SHL Zero Shift

```n1asm
shl_zero:
    MOV R0, #0x1234
    SHL R0, R0, #0x0000
    HALT
```

```n1test
R0 == 0x1234
```

## SHR Immediate

```n1asm
shr_imm:
    MOV R0, #0x0100
    SHR R0, R0, #0x0004
    HALT
```

```n1test
R0 == 0x0010
```

## SHR Maximum Shift

```n1asm
shr_max:
    MOV R0, #0x8000
    SHR R0, R0, #0x000F
    HALT
```

```n1test
R0 == 0x0001
```

## SHR Zero Shift

```n1asm
shr_zero:
    MOV R0, #0x1234
    SHR R0, R0, #0x0000
    HALT
```

```n1test
R0 == 0x1234
```

## CMP Does Not Modify Destination

CMP sets flags but does not write result to register.

```n1asm
cmp_no_write:
    MOV R0, #0x1234
    CMP R0, R0, #0x1234
    HALT
```

```n1test
R0 == 0x1234
```

## ADD Chain

Multiple ADDs in sequence.

```n1asm
add_chain:
    MOV R0, #0x0001
    ADD R0, R0, #0x0001
    ADD R0, R0, #0x0001
    ADD R0, R0, #0x0001
    HALT
```

```n1test
R0 == 0x0004
```

## Combined ALU Operations

Mix of different ALU operations.

```n1asm
alu_combo:
    MOV R0, #0x0001
    ADD R0, R0, #0x000F
    SHL R0, R0, #0x0004
    AND R0, R0, #0x01F0
    OR R0, R0, #0x000F
    HALT
```

```n1test
R0 == 0x010F
```

## AND All Bits Set

```n1asm
and_all_set:
    MOV R0, #0xFFFF
    AND R0, R0, #0xFFFF
    HALT
```

```n1test
R0 == 0xFFFF
```

## OR All Bits Clear

```n1asm
or_all_clear:
    MOV R0, #0x1234
    OR R0, R0, #0x0000
    HALT
```

```n1test
R0 == 0x1234
```
