# MMIO Instructions Test

Tests for MMIO operations (OP=0x8): IN, OUT.

Note: The test runner uses a NullMmio that returns 0 on reads and denies writes.
OUT operations will succeed but writes are suppressed.

## IN from MMIO

IN reads from MMIO address in R1. The test MMIO returns 0.

```n1asm
in_test:
    MOV R1, #0xE000
    IN R0, R1
    HALT
```

```n1test
R0 == 0x0000
```

## IN Different Address

```n1asm
in_addr:
    MOV R1, #0xE010
    IN R2, R1
    HALT
```

```n1test
R2 == 0x0000
```

## OUT to MMIO

OUT writes value in R0 to MMIO address in R1.

```n1asm
out_test:
    MOV R0, #0x1234
    MOV R1, #0xE020
    OUT R0, R1
    HALT
```

```n1test
R0 == 0x1234
```

## IN Preserves Address Register

```n1asm
in_preserve:
    MOV R1, #0xE030
    IN R2, R1
    HALT
```

```n1test
R1 == 0xE030
```

## Multiple IN Operations

```n1asm
multi_in:
    MOV R1, #0xE040
    IN R2, R1
    IN R3, R1
    HALT
```

```n1test
R2 == 0x0000
R3 == 0x0000
```
