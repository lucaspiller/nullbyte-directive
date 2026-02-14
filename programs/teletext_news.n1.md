# TELE-7 Teletext News Feed

A teletext-style news feed that automatically cycles through different pages.

## Program

```n1asm
; TELE-7 Teletext News Feed
; Auto-cycles through news pages

init:
    ; Enable TELE-7
    MOV R1, #0x0001
    STORE R1, #0xE122

    ; Set page buffer
    MOV R1, #0x4100
    STORE R1, #0xE124

    ; Initialize page counter
    MOV R4, #0

main:
    ; Clear screen
    MOV R5, #0x4100
    MOV R6, #0x2020
    MOV R7, #250

clear_loop:
    STORE R6, [R5]
    ADD R5, R5, #1
    SUB R7, R7, #1
    CMP R0, R7, R0
    JMP #clear_loop

    ; Check page number
    CMP R0, R4, #0
    JMP #page_0

    CMP R0, R4, #1
    JMP #page_1

    CMP R0, R4, #2
    JMP #page_2

    JMP #page_3

page_0:
    ; Header "PAGE 1"
    MOV R1, #0x5041
    STORE R1, #0x4100
    MOV R1, #0x4531
    STORE R1, #0x4101

    ; "BREAKING NEWS"
    MOV R1, #0x2042
    STORE R1, #0x4102
    MOV R1, #0x5245
    STORE R1, #0x4103
    MOV R1, #0x4141
    STORE R1, #0x4104
    MOV R1, #0x4E49
    STORE R1, #0x4105
    MOV R1, #0x4E47
    STORE R1, #0x4106
    MOV R1, #0x5345
    STORE R1, #0x4107
    MOV R1, #0x5757
    STORE R1, #0x4108
    MOV R1, #0x5300
    STORE R1, #0x4109

    JMP #wait_page

page_1:
    ; Header "PAGE 2"
    MOV R1, #0x5041
    STORE R1, #0x4100
    MOV R1, #0x4532
    STORE R1, #0x4101

    ; "WEATHER"
    MOV R1, #0x2057
    STORE R1, #0x4102
    MOV R1, #0x4541
    STORE R1, #0x4103
    MOV R1, #0x5448
    STORE R1, #0x4104
    MOV R1, #0x4552
    STORE R1, #0x4105

    JMP #wait_page

page_2:
    ; Header "PAGE 3"
    MOV R1, #0x5041
    STORE R1, #0x4100
    MOV R1, #0x4533
    STORE R1, #0x4101

    ; "SPORTS"
    MOV R1, #0x2053
    STORE R1, #0x4102
    MOV R1, #0x504F
    STORE R1, #0x4103
    MOV R1, #0x5254
    STORE R1, #0x4104
    MOV R1, #0x5300
    STORE R1, #0x4105

    JMP #wait_page

page_3:
    ; Header "PAGE 4"
    MOV R1, #0x5041
    STORE R1, #0x4100
    MOV R1, #0x4534
    STORE R1, #0x4101

    ; "MARKETS"
    MOV R1, #0x204D
    STORE R1, #0x4102
    MOV R1, #0x4152
    STORE R1, #0x4103
    MOV R1, #0x4B45
    STORE R1, #0x4104
    MOV R1, #0x5453
    STORE R1, #0x4105

    JMP #wait_page

wait_page:
    MOV R1, #30
wait_loop:
    SUB R1, R1, #1
    CMP R0, R1, R0
    JMP #wait_loop

    ADD R4, R4, #1
    CMP R0, R4, #4
    JMP #wrap
    JMP #main

wrap:
    MOV R4, #0
    JMP #main
```
