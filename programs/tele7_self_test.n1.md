# TELE-7 Self-Test Program

This program tests the TELE-7 textual display device.

## MMIO Addresses

- 0xE120: ID
- 0xE121: VERSION
- 0xE122: CTRL
- 0xE123: STATUS
- 0xE124: PAGE_BASE
- 0xE125: BORDER
- 0xE126: ORIGIN
- 0xE127: BLINK_DIV

## Test Setup

```n1asm
init:
    MOV R1, #0x4100
    STORE R1, #0xE124    ; PAGE_BASE

    MOV R1, #0x0001
    STORE R1, #0xE122    ; CTRL = enabled

    HALT
```

## Test 1: VERSION

```n1asm
test_version:
    LOAD R4, #0xE121
    HALT
```

## Test 2: Character Write

```n1asm
test_char:
    MOV R1, #0x4142
    STORE R1, #0x4100
    HALT
```

## Test 3: Colors

```n1asm
test_colors:
    MOV R1, #0x0152
    STORE R1, #0x4114
    HALT
```

## Test 4: Mosaic

```n1asm
test_mosaic:
    MOV R1, #0x1820
    STORE R1, #0x4164
    HALT
```

## Test 5: Scroll

```n1asm
test_scroll:
    MOV R1, #0x544F
    STORE R1, #0x4100
    MOV R1, #0x0005
    STORE R1, #0xE126
    HALT
```

## Test 6: Border

```n1asm
test_border:
    MOV R1, #0x0003
    STORE R1, #0xE125
    LOAD R3, #0xE125
    HALT
```

```n1test
R3 == 0x0003
```

## Test 7: BLINK_DIV

```n1asm
test_blink:
    MOV R1, #0x0019
    STORE R1, #0xE127
    HALT
```

## Test 8: Disable

```n1asm
test_disable:
    MOV R1, #0x0000
    STORE R1, #0xE122
    LOAD R5, #0xE123
    HALT
```

```n1test
R5 == 0x0000
```

## Test 9: Re-enable

```n1asm
test_enable:
    MOV R1, #0x0001
    STORE R1, #0xE122
    MOV R1, #0x4F4B
    STORE R1, #0x4100
    HALT
```

## End

```n1asm
done:
    HALT
```
