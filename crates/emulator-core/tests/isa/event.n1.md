# Event Instructions Test

Tests for event operations (OP=0xA): EWAIT, EGET, ERET.

Note: The test runner uses an empty event queue, so:

- EWAIT will spin (PC stays the same, then budget exhausts)
- EGET will return 0 (no events)

## EGET Empty Queue

EGET with empty queue returns 0 in destination.

```n1asm
eget_empty:
    EGET R0
    HALT
```

```n1test
R0 == 0x0000
```

## EGET Different Register

```n1asm
eget_reg:
    EGET R2
    HALT
```

```n1test
R2 == 0x0000
```

## EGET Multiple Calls

```n1asm
eget_multi:
    EGET R0
    EGET R1
    EGET R2
    HALT
```

```n1test
R0 == 0x0000
R1 == 0x0000
R2 == 0x0000
```

## EGET Does Not Affect Other Registers

```n1asm
eget_preserve:
    MOV R3, #0x1234
    EGET R4
    HALT
```

```n1test
R3 == 0x1234
R4 == 0x0000
```

## EGET Then Operation

```n1asm
eget_then_op:
    EGET R0
    ADD R0, R0, #0x0001
    HALT
```

```n1test
R0 == 0x0001
```
