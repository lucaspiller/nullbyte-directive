# Atomic MMIO Instructions Test

Tests for atomic MMIO operations (OP=0x9): BSET, BCLR, BTEST.

Note: The test runner uses a NullMmio that returns 0 on reads and denies writes.
BSET/BCLR writes are suppressed, but flags are still set based on the read
value.

## BSET Bit 0

BSET sets a bit in MMIO. Test MMIO returns 0.

```n1asm
bset_0:
    MOV R1, #0xE000
    BSET R1, #0x0000
    HALT
```

```n1test
R1 == 0xE000
```

## BSET Bit 4

```n1asm
bset_4:
    MOV R1, #0xE010
    BSET R1, #0x0004
    HALT
```

```n1test
R1 == 0xE010
```

## BCLR Bit 0

BCLR clears a bit in MMIO.

```n1asm
bclr_0:
    MOV R1, #0xE020
    BCLR R1, #0x0000
    HALT
```

```n1test
R1 == 0xE020
```

## BTEST on Zero Value

BTEST tests a bit without modifying. With MMIO returning 0, Z flag should be
set.

```n1asm
btest_zero:
    MOV R1, #0xE030
    BTEST R1, #0x0000
    HALT
```

```n1test
R1 == 0xE030
```

## BTEST Different Bit

```n1asm
btest_bit:
    MOV R1, #0xE040
    BTEST R1, #0x0008
    HALT
```

```n1test
R1 == 0xE040
```

## Multiple BSET Operations

```n1asm
multi_bset:
    MOV R1, #0xE050
    BSET R1, #0x0001
    BSET R1, #0x0002
    HALT
```

```n1test
R1 == 0xE050
```
