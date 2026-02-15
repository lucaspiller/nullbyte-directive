# TELE-7 Directives Demo

This program demonstrates the new TELE-7 authoring directives.

```n1asm
; TELE-7 MMIO registers
; 0xE122 CTRL
; 0xE124 PAGE_BASE
; 0xE125 BORDER

init:
    ; Map display page at 0x4100 and enable
    MOV R1, #0x4100
    STORE R1, #0xE124
    MOV R1, #0x0001
    STORE R1, #0xE122

    ; Banner using .tstring directive
    ; "HELLO" packed as words: 0x4845 0x4C4C 0x4F20
    .org 0x4100
    .tstring "HELLO"

    ; Red foreground + black background using .twchar with control tokens
    ; $FG1 = 0x01 (red), $BG0 = 0x10 (black)
    .org 0x4120
    .twchar $FG1, $BG0
    .tstring "RED TEXT"

    ; Flashing text
    .org 0x4140
    .twchar $FLASH_ON, ' '
    .tstring "ALERT!"

    ; Mosaic graphics demo
    .org 0x4160
    .twchar $MOSAIC_ON, ' '
    .twchar 0xDB, 0xDB  ; Full block characters

main:
    HALT
    JMP #main
```

## Test assertions

After initialization, check the page buffer contents.

```n1test
; Banner at 0x4100-0x4104: "HE" "LL" "O "
[0x4100] == 0x48
[0x4101] == 0x45
[0x4102] == 0x4C
[0x4103] == 0x4C
[0x4104] == 0x4F
```
