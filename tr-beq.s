; in = xE
; out = xD

read:

1000'00B7   lui x1, #1000'0         1000'0 08+37
0010'0113   addi x2, x0, #01        001 00+0 10+13
0000'0737   lui xE, 0               0000'0 70+37
FFC0'0293   addi x5, x0, -4         FFC 00+0 28+13
0000'000F   fence                   0 0 0 00+0 00+0F
0050'8203   lb x4, 5(x1)            005 08+0 20+03
0022'7233   and x4, x4, x2          00+02 20+7 20+33
FE02'0CE3   beq x4, x0, -2insn      FE+00 20+0 C8+63
0000'000F   fence                   0 0 0 00+0 00+0F
0000'8783   lb xF, 0(x1)            000 08+0 78+03
0187'9793   slli xF, xF, #18        018 78+1 78+13
0087'5713   srli xE, xE, #8         008 70+5 70+13
00F7'6733   or xE, xE, xF           00+0F 70+6 70+33
0012'8293   addi x5, x5, 1          001 28+0 28+13
FC02'CCE3   blt x5, x0, -10insn     FC+00 28+4 C8+63

tr_beq:

            addi out, x0, #n1100011
            ; rs1 (r1) -> 19:15
            slli xF, in, #8 ; 31-23
            srli xF, xF, #28 ; 31-23+20
            slli xF, xF, 15
            or out, out, xF
            ; rs2 (r2) -> 24:20
            slli xF, in, 31-19
            srli xF, xF, 31-19+16
            slli xF, xF, 20
            or out, out, xF
            ; imm[11-2] -> 7
            slli xF, in, 31-9
            srli xF, xF, 31-9+9
            slli xF, xF, 7
            or out, out, xF
            ; imm[4-2:1-1] -> 11:9
            slli xF, in, 31-2
            srli xF, xF, 31-2+0
            slli xF, xF, 9
            or out, out, xF
            ; imm[10-2:5-2] -> 30:25
            slli xF, in, 31-8
            srli xF, xF, 31-8+3
            slli xF, xF, 25
            or out, out, xF
            ; imm[12] -> 31
            slli xF, in, 31-12
            srli xF, xF, 31-12+12
            slli xF, xF, 31
            or out, out, xF
            ; send
            ;jalr x0, 0(ra)

0000'0000   inval

write:

;0200'0113   addi x2, x0, #20        020 00+0 10+13
;0000'000F   fence                   0 0 0 00+0 00+0F
;0050'8203   lb x4, 5(x1)            005 08+0 20+03
;0022'7233   and x4, x4, x2          00+02 20+7 20+33
;FE02'0CE3   beq x4, x0, -2insn      FE+00 20+0 C8+63
;0000'000F   fence                   0 0 0 00+0 00+0F
;0030'8023   sb x3, 0(x1)            00+03 08+0 00+23
;
;
;8000'07B7   lui xF, #8000'0         8000'0 78+37
;            jalr x0, 0(xF)
;0000'0000   inval


; 26 insns, so 7 bits (128 bytes, 32 insns) is fine
