;;;
$start
            ; set up regular stack pointer
            lui sp #8001'1

            ; set up interrupt stack pointer
            lui t0 #8001'0
            csrrw x0 t0 #340 ; mscratch

            ; set up mtvec
            lui t0 #8000'0 ; #8000'0030 vectored
            addi t0 t0 #031
            csrrw x0 t0 #305 ; mtvec
            csrrs t1 x0 #305
            bne t0 t1 bad_mtvec

            jal x0 vec_table_end

            nop
            nop
            nop

$vec_table ; #8000'0030
            jal x0 exception ; no user soft interrupts rn
            jal x0 supervisor_soft_int
            jal x0 unknown_int
            jal x0 machine_soft_int

            jal x0 user_timer_int
            jal x0 supervisor_timer_int
            jal x0 unknown_int
            jal x0 machine_timer_int

            jal x0 user_ext_int
            jal x0 supervisor_ext_int
            jal x0 unknown_int
            jal x0 machine_ext_int
$vec_table_end

            ; induce an exception
            lw x0 x0 #0
            jal x0 shutdown

$bad_mtvec
            addi s0 t1 #0

            addi a0 x0 #40 ; @
            jal ra write
            addi a0 x0 #76 ; v
            jal ra write

            addi a0 s0 #0
            jal ra write_hex_u32

            jal ra crlf

            jal x0 shutdown

;;;
$exception
            csrrw sp sp #340 ; mscratch
            addi sp sp #40

            sw ra sp -#40

            sw t0 sp -#3C
            sw t1 sp -#38
            sw t2 sp -#34
            sw t3 sp -#30
            sw t4 sp -#2C
            sw t5 sp -#28
            sw t6 sp -#24

            sw a0 sp -#20
            sw a1 sp -#1C
            sw a2 sp -#18
            sw a3 sp -#14
            sw a4 sp -#10
            sw a5 sp -#0C
            sw a6 sp -#08
            sw a7 sp -#04

            ; branch to mcause handler
            csrrs t0 x0 #342 ; mcause
            slli t0 t0 #2
            auipc t1 #0
            add t0 t0 t1
            jalr x0 t0 #C

            jal x0 die_from_exception ; instruction address misaligned
            jal x0 die_from_exception ; instruction access fault
            jal x0 die_from_exception ; illegal instruction
            jal x0 die_from_exception ; breakpoint

            jal x0 die_from_exception ; load address misaligned
            jal x0 die_from_exception ; load access fault
            jal x0 die_from_exception ; store/AMO address misaligned
            jal x0 die_from_exception ; store/AMO access fault

            jal x0 die_from_exception ; ecall from user mode
            jal x0 die_from_exception ; ecall from supervisor mode
            jal x0 die_from_exception ; (reserved)
            jal x0 die_from_exception ; ecall from machine mode

            jal x0 die_from_exception ; instruction page fault
            jal x0 die_from_exception ; load page fault
            jal x0 die_from_exception ; (reserved)
            jal x0 die_from_exception ; store/AMO page fault

$exception_end
            lw ra sp -#40

            lw t0 sp -#3C
            lw t1 sp -#38
            lw t2 sp -#34
            lw t3 sp -#30
            lw t4 sp -#2C
            lw t5 sp -#28
            lw t6 sp -#24

            lw a0 sp -#20
            lw a1 sp -#1C
            lw a2 sp -#18
            lw a3 sp -#14
            lw a4 sp -#10
            lw a5 sp -#0C
            lw a6 sp -#08
            lw a7 sp -#04

            addi sp sp -#40
            csrrw sp sp #340 ; mscratch
            mret

$die_from_exception
            ; write mcause
            addi a0 x0 #40 ; @
            jal ra write
            addi a0 x0 #6D ; m
            jal ra write
            addi a0 x0 #23 ; #
            jal ra write
            csrrs a0 x0 #342 ; mcause
            jal ra write_hex_u32
            jal ra crlf
            jal x0 shutdown


;;;
$supervisor_soft_int
$machine_soft_int
$user_timer_int
$supervisor_timer_int
$machine_timer_int
$user_ext_int
$supervisor_ext_int
$machine_ext_int
            addi a0 x0 #40 ; @
            jal ra write
            addi a0 x0 #69 ; i (nice)
            jal ra write

            jal ra crlf

            jal x0 shutdown

;;;
$unknown_int
            addi a0 x0 #40 ; @
            jal ra write
            addi a0 x0 #3F ; ?
            jal ra write
            csrrs a0 x0 #342 ; mcause
            jal ra write_hex_u32
            jal ra crlf

            jal x0 shutdown

;;;
$shutdown
            lui t1 #100
            lui t0 #5 ; #5555
            addi t0 t0 #555
            sw t0 t1 #0
            jal x0 #0


;;;
$write_hex_u32
            addi sp sp #8
            sw ra sp -#8
            sw s0 sp -#4

            addi s0 a0 #0

            srli a0 s0 #10
            jal ra write_hex_u16
            addi a0 x0 #27 ; '
            jal ra write
            slli a0 s0 #10
            srli a0 a0 #10
            jal ra write_hex_u16

            lw s0 sp -#4
            lw ra sp -#8
            addi sp sp -#8
            ret


;;;
$write_hex_u16
            addi sp sp #8
            sw ra sp -#8
            sw s0 sp -#4

            addi s0 a0 #0

            srli a0 s0 #C
            jal ra u4_to_hex
            jal ra write
            srli a0 s0 #8
            andi a0 a0 #F
            jal ra u4_to_hex
            jal ra write
            srli a0 s0 #4
            andi a0 a0 #F
            jal ra u4_to_hex
            jal ra write
            andi a0 s0 #F
            jal ra u4_to_hex
            jal ra write

            lw s0 sp -#4
            lw ra sp -#8
            addi sp sp -#8
            ret


;;;
$u4_to_hex
            addi t0 x0 #A
            blt a0 t0 lt_A
            addi a0 a0 #37
            ret
$lt_A
            addi a0 a0 #30
            ret


;;;
$crlf
            addi sp sp #4
            sw ra sp -#4

            addi a0 x0 #0D ; CR
            jal ra write
            addi a0 x0 #0A ; LF
            jal ra write

            lw ra sp -#4
            addi sp sp -#4
            ret


;;;
$write
            lui t0 #1000'0
            addi t1 x0 #20
            lb t2 t0 #5
            and t2 t2 t1
            beq t2 x0 -#8
            sb a0 t0 #0
            ret
