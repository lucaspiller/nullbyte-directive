# Branch Instructions Test

Tests for branch operations (OP=0x6): BEQ, BNE, BLT, BLE, BGT, BGE, JMP,
CALL/RET.

## JMP Forward

```n1asm
jmp_test:
    MOV R0, #0x0000
    JMP #jt_skip
    MOV R0, #0xFFFF
jt_skip:
    HALT
```

```n1test
R0 == 0x0000
```

## BEQ Taken

```n1asm
beq_test:
    MOV R0, #0x0005
    CMP R1, R0, #0x0005
    BEQ #bt_yes
    MOV R0, #0x0000
bt_yes:
    HALT
```

```n1test
R0 == 0x0005
```

## BNE Taken

```n1asm
bne_test:
    MOV R0, #0x0005
    CMP R1, R0, #0x0006
    BNE #bnt_yes
    MOV R0, #0x0000
bnt_yes:
    HALT
```

```n1test
R0 == 0x0005
```

## BLT Negative Result

```n1asm
blt_test:
    MOV R0, #0x0005
    CMP R1, R0, #0x000A
    BLT #blt_yes
    MOV R0, #0x0000
blt_yes:
    HALT
```

```n1test
R0 == 0x0005
```

## BGE Positive

```n1asm
bge_test:
    MOV R0, #0x000A
    CMP R1, R0, #0x0005
    BGE #bgt_yes
    MOV R0, #0x0000
bgt_yes:
    HALT
```

```n1test
R0 == 0x000A
```
