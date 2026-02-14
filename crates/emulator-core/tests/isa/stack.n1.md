# Stack Operations Test

Tests for stack operations (OP=0x7): PUSH, POP.

Note: Stack tests are limited because the stack pointer state persists between
tests. Only basic functionality is tested here.

## PUSH and POP Same Value

```n1asm
push_pop_basic:
    MOV R0, #0x1234
    PUSH R0
    POP R1
    HALT
```

```n1test
R0 == 0x1234
R1 == 0x1234
```

## PUSH Preserves Source Register

```n1asm
push_preserve:
    MOV R2, #0xABCD
    PUSH R2
    HALT
```

```n1test
R2 == 0xABCD
```
