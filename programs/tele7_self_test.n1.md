# TELE-7 Self-Test Program (Looping)

A richer TELE-7 exercise program that configures the device, writes multiple
regions of text, and continuously updates visual state so behavior is obvious in
human testing.

```n1asm
; TELE-7 MMIO
; 0xE122 CTRL
; 0xE123 STATUS
; 0xE124 PAGE_BASE
; 0xE125 BORDER
; 0xE126 ORIGIN
; 0xE127 BLINK_DIV

init:
    ; Map display page and enable output
    MOV R1, #0x4100
    STORE R1, #0xE124
    MOV R1, #0x0001
    STORE R1, #0xE122

    ; Blink faster for visual confirmation
    MOV R1, #0x0008
    STORE R1, #0xE127

    ; --- Static banner ---
    ; "TELE-7 SELF TEST LOOP"
    MOV R1, #0x5445 ; TE
    STORE R1, #0x4100
    MOV R1, #0x4C45 ; LE
    STORE R1, #0x4102
    MOV R1, #0x2D37 ; -7
    STORE R1, #0x4104
    MOV R1, #0x2053 ;  S
    STORE R1, #0x4106
    MOV R1, #0x454C ; EL
    STORE R1, #0x4108
    MOV R1, #0x4620 ; F
    STORE R1, #0x410A
    MOV R1, #0x5445 ; TE
    STORE R1, #0x410C
    MOV R1, #0x5354 ; ST
    STORE R1, #0x410E
    MOV R1, #0x204C ;  L
    STORE R1, #0x4110
    MOV R1, #0x4F4F ; OO
    STORE R1, #0x4112
    MOV R1, #0x5000 ; P.
    STORE R1, #0x4114

    ; "MMIO STATUS BORDER ORIGIN"
    MOV R1, #0x4D4D ; MM
    STORE R1, #0x4150
    MOV R1, #0x494F ; IO
    STORE R1, #0x4152
    MOV R1, #0x2053 ;  S
    STORE R1, #0x4154
    MOV R1, #0x5441 ; TA
    STORE R1, #0x4156
    MOV R1, #0x5455 ; TU
    STORE R1, #0x4158
    MOV R1, #0x5320 ; S
    STORE R1, #0x415A
    MOV R1, #0x424F ; BO
    STORE R1, #0x415C
    MOV R1, #0x5244 ; RD
    STORE R1, #0x415E
    MOV R1, #0x4552 ; ER
    STORE R1, #0x4160
    MOV R1, #0x204F ;  O
    STORE R1, #0x4162
    MOV R1, #0x5249 ; RI
    STORE R1, #0x4164
    MOV R1, #0x4749 ; GI
    STORE R1, #0x4166
    MOV R1, #0x4E00 ; N.
    STORE R1, #0x4168

    ; "SPINNER:"
    MOV R1, #0x5350 ; SP
    STORE R1, #0x41A0
    MOV R1, #0x494E ; IN
    STORE R1, #0x41A2
    MOV R1, #0x4E45 ; NE
    STORE R1, #0x41A4
    MOV R1, #0x523A ; R:
    STORE R1, #0x41A6

    ; "CTRL SHOULD STAY ON"
    MOV R1, #0x4354 ; CT
    STORE R1, #0x41F0
    MOV R1, #0x524C ; RL
    STORE R1, #0x41F2
    MOV R1, #0x2053 ;  S
    STORE R1, #0x41F4
    MOV R1, #0x484F ; HO
    STORE R1, #0x41F6
    MOV R1, #0x554C ; UL
    STORE R1, #0x41F8
    MOV R1, #0x4420 ; D
    STORE R1, #0x41FA
    MOV R1, #0x5354 ; ST
    STORE R1, #0x41FC
    MOV R1, #0x4159 ; AY
    STORE R1, #0x41FE
    MOV R1, #0x204F ;  O
    STORE R1, #0x4200
    MOV R1, #0x4E00 ; N.
    STORE R1, #0x4202

    MOV R4, #0x0000 ; phase 0..7

main_loop:
    ; Read STATUS and show in row 2 as "S=XX"
    LOAD R2, #0xE123

    MOV R1, #0x533D ; S=
    STORE R1, #0x4190

    ; Hex lookup by nibble using phase buckets (simple visual marker)
    ; We show a changing marker next to status to prove loop activity.
    CMP R0, R4, #0
    BEQ #spin_bar
    CMP R0, R4, #1
    BEQ #spin_slash
    CMP R0, R4, #2
    BEQ #spin_dash
    CMP R0, R4, #3
    BEQ #spin_backslash
    CMP R0, R4, #4
    BEQ #spin_bar
    CMP R0, R4, #5
    BEQ #spin_slash
    CMP R0, R4, #6
    BEQ #spin_dash
    JMP #spin_backslash

spin_bar:
    MOV R1, #0x7C20 ; "| "
    STORE R1, #0x41A8
    JMP #phase_side_effects

spin_slash:
    MOV R1, #0x2F20 ; "/ "
    STORE R1, #0x41A8
    JMP #phase_side_effects

spin_dash:
    MOV R1, #0x2D20 ; "- "
    STORE R1, #0x41A8
    JMP #phase_side_effects

spin_backslash:
    MOV R1, #0x5C20 ; "\\ "
    STORE R1, #0x41A8

phase_side_effects:
    ; BORDER = phase & 0x7
    STORE R4, #0xE125

    ; ORIGIN = phase (small scroll pulse)
    STORE R4, #0xE126

    ; Write phase digit at a fixed position
    CMP R0, R4, #0
    BEQ #digit_0
    CMP R0, R4, #1
    BEQ #digit_1
    CMP R0, R4, #2
    BEQ #digit_2
    CMP R0, R4, #3
    BEQ #digit_3
    CMP R0, R4, #4
    BEQ #digit_4
    CMP R0, R4, #5
    BEQ #digit_5
    CMP R0, R4, #6
    BEQ #digit_6
    JMP #digit_7

digit_0:
    MOV R1, #0x5030 ; P0
    STORE R1, #0x4192
    JMP #delay

digit_1:
    MOV R1, #0x5031 ; P1
    STORE R1, #0x4192
    JMP #delay

digit_2:
    MOV R1, #0x5032 ; P2
    STORE R1, #0x4192
    JMP #delay

digit_3:
    MOV R1, #0x5033 ; P3
    STORE R1, #0x4192
    JMP #delay

digit_4:
    MOV R1, #0x5034 ; P4
    STORE R1, #0x4192
    JMP #delay

digit_5:
    MOV R1, #0x5035 ; P5
    STORE R1, #0x4192
    JMP #delay

digit_6:
    MOV R1, #0x5036 ; P6
    STORE R1, #0x4192
    JMP #delay

digit_7:
    MOV R1, #0x5037 ; P7
    STORE R1, #0x4192

delay:
    MOV R6, #0x1200

delay_loop:
    SUB R6, R6, #1
    CMP R0, R6, #0
    BNE #delay_loop

    ADD R4, R4, #1
    CMP R0, R4, #8
    BLT #main_loop
    MOV R4, #0
    JMP #main_loop
```
