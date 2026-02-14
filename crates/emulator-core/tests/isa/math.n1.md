# Math Helper Instructions Test

Tests for math operations (OP=0x5): MUL, MULH, DIV, MOD, QADD, QSUB, SCV.

## MUL Immediate

```n1asm
mul_imm:
    MOV R0, #0x0010
    MUL R0, R0, #0x0010
    HALT
```

```n1test
R0 == 0x0100
```

## MUL Zero

```n1asm
mul_zero:
    MOV R0, #0x1234
    MUL R0, R0, #0x0000
    HALT
```

```n1test
R0 == 0x0000
```

## MUL Overflow (Low 16 bits)

```n1asm
mul_overflow:
    MOV R0, #0x0100
    MUL R0, R0, #0x0100
    HALT
```

```n1test
R0 == 0x0000
```

## MULH High Bits

MULH returns the high 16 bits of the 32-bit product.

```n1asm
mulh_basic:
    MOV R0, #0x0100
    MULH R1, R0, #0x0100
    HALT
```

```n1test
R1 == 0x0001
```

## MULH Small Values

```n1asm
mulh_small:
    MOV R0, #0x0010
    MULH R1, R0, #0x0010
    HALT
```

```n1test
R1 == 0x0000
```

## DIV Basic

```n1asm
div_basic:
    MOV R0, #0x0100
    DIV R0, R0, #0x0010
    HALT
```

```n1test
R0 == 0x0010
```

## DIV Truncates

```n1asm
div_truncate:
    MOV R0, #0x0017
    DIV R0, R0, #0x0005
    HALT
```

```n1test
R0 == 0x0004
```

## DIV By Zero

Division by zero returns 0.

```n1asm
div_zero:
    MOV R0, #0x1234
    DIV R0, R0, #0x0000
    HALT
```

```n1test
R0 == 0x0000
```

## MOD Basic

```n1asm
mod_basic:
    MOV R0, #0x0017
    MOD R0, R0, #0x0005
    HALT
```

```n1test
R0 == 0x0003
```

## MOD Divisible

```n1asm
mod_divisible:
    MOV R0, #0x0100
    MOD R0, R0, #0x0010
    HALT
```

```n1test
R0 == 0x0000
```

## MOD By Zero

Modulo by zero returns 0.

```n1asm
mod_zero:
    MOV R0, #0x1234
    MOD R0, R0, #0x0000
    HALT
```

```n1test
R0 == 0x0000
```

## QADD No Saturate

Saturating add without overflow.

```n1asm
qadd_no_sat:
    MOV R0, #0x1000
    QADD R0, R0, #0x2000
    HALT
```

```n1test
R0 == 0x3000
```

## QADD Saturate Positive

Saturating add that would overflow positive.

```n1asm
qadd_sat_pos:
    MOV R0, #0x7FFF
    QADD R0, R0, #0x0001
    HALT
```

```n1test
R0 == 0x7FFF
```

## QSUB No Saturate

Saturating sub without underflow.

```n1asm
qsub_no_sat:
    MOV R0, #0x3000
    QSUB R0, R0, #0x2000
    HALT
```

```n1test
R0 == 0x1000
```

## QSUB Saturate Negative

Saturating sub that would underflow.

```n1asm
qsub_sat_neg:
    MOV R0, #0x8000
    QSUB R0, R0, #0x0001
    HALT
```

```n1test
R0 == 0x8000
```

## SCV Sign Convert

SCV converts the value treating it as signed (identity operation for bit
pattern).

```n1asm
scv_basic:
    MOV R0, #0x1234
    SCV R1, R0
    HALT
```

```n1test
R1 == 0x1234
```

## MUL Chain

Multiple multiplications.

```n1asm
mul_chain:
    MOV R0, #0x0002
    MUL R0, R0, #0x0003
    MUL R0, R0, #0x0004
    HALT
```

```n1test
R0 == 0x0018
```

## DIV and MOD Relationship

```n1asm
div_mod_rel:
    MOV R0, #0x0017
    MOV R1, R0
    DIV R0, R0, #0x0005
    MOD R1, R1, #0x0005
    HALT
```

```n1test
R0 == 0x0004
R1 == 0x0003
```

## MUL Max Values

```n1asm
mul_max:
    MOV R0, #0x00FF
    MUL R0, R0, #0x0100
    HALT
```

```n1test
R0 == 0xFF00
```
