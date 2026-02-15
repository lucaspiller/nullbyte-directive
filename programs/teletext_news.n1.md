# TELE-7 Teletext News Feed (Looping)

A larger teletext-style news demo that rotates pages forever and updates visual
state each cycle.

```n1asm
; TELE-7 MMIO
; 0xE122 CTRL
; 0xE124 PAGE_BASE
; 0xE125 BORDER
; 0xE126 ORIGIN
; 0xE127 BLINK_DIV

init:
    ; Enable display and page map
    MOV R1, #0x4100
    STORE R1, #0xE124
    MOV R1, #0x0001
    STORE R1, #0xE122

    ; Medium blink pace
    MOV R1, #0x0010
    STORE R1, #0xE127

    ; Static masthead: "NBD NEWSNET"
    MOV R1, #0x4E42 ; NB
    STORE R1, #0x4100
    MOV R1, #0x4420 ; D
    STORE R1, #0x4102
    MOV R1, #0x4E45 ; NE
    STORE R1, #0x4104
    MOV R1, #0x5753 ; WS
    STORE R1, #0x4106
    MOV R1, #0x4E45 ; NE
    STORE R1, #0x4108
    MOV R1, #0x5400 ; T.
    STORE R1, #0x410A

    ; Footer label: "PAGE"
    MOV R1, #0x5041 ; PA
    STORE R1, #0x4C00
    MOV R1, #0x4745 ; GE
    STORE R1, #0x4C02
    MOV R1, #0x3A20 ; :
    STORE R1, #0x4C04

    MOV R4, #0 ; page index 0..3

news_loop:
    ; update border + slight scroll to show liveness
    STORE R4, #0xE125
    STORE R4, #0xE126

    ; clear a few content rows we overwrite (spaces)
    MOV R1, #0x2020
    STORE R1, #0x4140
    STORE R1, #0x4142
    STORE R1, #0x4144
    STORE R1, #0x4146
    STORE R1, #0x4148
    STORE R1, #0x414A
    STORE R1, #0x414C
    STORE R1, #0x414E

    STORE R1, #0x4180
    STORE R1, #0x4182
    STORE R1, #0x4184
    STORE R1, #0x4186
    STORE R1, #0x4188
    STORE R1, #0x418A
    STORE R1, #0x418C
    STORE R1, #0x418E

    STORE R1, #0x41C0
    STORE R1, #0x41C2
    STORE R1, #0x41C4
    STORE R1, #0x41C6
    STORE R1, #0x41C8
    STORE R1, #0x41CA
    STORE R1, #0x41CC
    STORE R1, #0x41CE

    ; Page selector
    CMP R0, R4, #0
    BEQ #page_1
    CMP R0, R4, #1
    BEQ #page_2
    CMP R0, R4, #2
    BEQ #page_3
    JMP #page_4

page_1:
    ; PAGE:1
    MOV R1, #0x3120 ; 1
    STORE R1, #0x4C06

    ; "BREAKING"
    MOV R1, #0x4252 ; BR
    STORE R1, #0x4140
    MOV R1, #0x4541 ; EA
    STORE R1, #0x4142
    MOV R1, #0x4B49 ; KI
    STORE R1, #0x4144
    MOV R1, #0x4E47 ; NG
    STORE R1, #0x4146

    ; "CORE STABLE"
    MOV R1, #0x434F ; CO
    STORE R1, #0x4180
    MOV R1, #0x5245 ; RE
    STORE R1, #0x4182
    MOV R1, #0x2053 ;  S
    STORE R1, #0x4184
    MOV R1, #0x5441 ; TA
    STORE R1, #0x4186
    MOV R1, #0x424C ; BL
    STORE R1, #0x4188
    MOV R1, #0x4500 ; E.
    STORE R1, #0x418A

    JMP #wait_and_next

page_2:
    ; PAGE:2
    MOV R1, #0x3220 ; 2
    STORE R1, #0x4C06

    ; "WEATHER"
    MOV R1, #0x5745 ; WE
    STORE R1, #0x4140
    MOV R1, #0x4154 ; AT
    STORE R1, #0x4142
    MOV R1, #0x4845 ; HE
    STORE R1, #0x4144
    MOV R1, #0x5200 ; R.
    STORE R1, #0x4146

    ; "CLEAR SKIES"
    MOV R1, #0x434C ; CL
    STORE R1, #0x4180
    MOV R1, #0x4541 ; EA
    STORE R1, #0x4182
    MOV R1, #0x5220 ; R
    STORE R1, #0x4184
    MOV R1, #0x534B ; SK
    STORE R1, #0x4186
    MOV R1, #0x4945 ; IE
    STORE R1, #0x4188
    MOV R1, #0x5300 ; S.
    STORE R1, #0x418A

    JMP #wait_and_next

page_3:
    ; PAGE:3
    MOV R1, #0x3320 ; 3
    STORE R1, #0x4C06

    ; "MARKETS"
    MOV R1, #0x4D41 ; MA
    STORE R1, #0x4140
    MOV R1, #0x524B ; RK
    STORE R1, #0x4142
    MOV R1, #0x4554 ; ET
    STORE R1, #0x4144
    MOV R1, #0x5300 ; S.
    STORE R1, #0x4146

    ; "UP 2 POINTS"
    MOV R1, #0x5550 ; UP
    STORE R1, #0x4180
    MOV R1, #0x2032 ;  2
    STORE R1, #0x4182
    MOV R1, #0x2050 ;  P
    STORE R1, #0x4184
    MOV R1, #0x4F49 ; OI
    STORE R1, #0x4186
    MOV R1, #0x4E54 ; NT
    STORE R1, #0x4188
    MOV R1, #0x5300 ; S.
    STORE R1, #0x418A

    JMP #wait_and_next

page_4:
    ; PAGE:4
    MOV R1, #0x3420 ; 4
    STORE R1, #0x4C06

    ; "SPORTS"
    MOV R1, #0x5350 ; SP
    STORE R1, #0x4140
    MOV R1, #0x4F52 ; OR
    STORE R1, #0x4142
    MOV R1, #0x5453 ; TS
    STORE R1, #0x4144

    ; "HOME WIN"
    MOV R1, #0x484F ; HO
    STORE R1, #0x4180
    MOV R1, #0x4D45 ; ME
    STORE R1, #0x4182
    MOV R1, #0x2057 ;  W
    STORE R1, #0x4184
    MOV R1, #0x494E ; IN
    STORE R1, #0x4186

wait_and_next:
    ; pacing delay
    MOV R6, #0x1800
news_delay:
    SUB R6, R6, #1
    CMP R0, R6, #0
    BNE #news_delay

    ADD R4, R4, #1
    CMP R0, R4, #4
    BLT #news_loop
    MOV R4, #0
    JMP #news_loop
```
