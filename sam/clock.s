            lui sp #8000'1

$get_time
            lui t0 #200'C ; #200'BFF8
$get_time_retry
            lw s1 t0 #FFC
            lw s0 t0 #FF8
            lw t1 t0 #FFC
            bne s1 t1 get_time_retry

$loop
            ; {s1,s0} += #98'9680
            lui t0 #98'9
            addi t0 t0 #680
            add t1 s0 t0
            sltu t2 t1 s0
            addi s0 t1 #0
            add s1 s1 t2

            addi a0 s0 #0
            addi a1 s1 #0
            jal ra delay_time

            addi a0 s1 #0
            jal ra write_hex_u32
            addi a0 x0 #27
            jal ra write
            addi a0 s0 #0
            jal ra write_hex_u32
            jal ra crlf

            jal x0 loop

            jal x0 shutdown


$shutdown
            lui t1 #100
            lui t0 #5
            addi t0 t0 #555
            sw t0 t1 #0

            inval

$delay_time
            lui t3 #200'C
$delay_time_retry
            lw t1 t3 #FFC
            lw t0 t3 #FF8
            lw t2 t3 #FFC
            bne t1 t2 delay_time_retry
            bltu a1 t1 delay_time_ret
            bltu t1 a1 delay_time_retry
            bltu t0 a0 delay_time_retry
$delay_time_ret
            ret


$write_hex_u32
            addi sp sp #8
            sw ra sp -#8
            sw s0 sp -#4

            addi s0 a0 #0

            srli a0 s0 #10
            jal ra write_hex_u16
            addi a0 x0 #27
            jal ra write
            slli a0 s0 #10
            srli a0 a0 #10
            jal ra write_hex_u16

            lw s0 sp -#4
            lw ra sp -#8
            addi sp sp -#8
            ret


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


$u4_to_hex
            addi t0 x0 #A
            blt a0 t0 lt_A
            addi a0 a0 #37
            ret
$lt_A
            addi a0 a0 #30
            ret


$crlf
            addi sp sp #4
            sw ra sp -#4

            addi a0 x0 #0D
            jal ra write
            addi a0 x0 #0A
            jal ra write

            lw ra sp -#4
            addi sp sp -#4
            ret


$write
            lui t0 #1000'0
            addi t1 x0 #20
            fence io i
            lb t2 t0 #5
            and t2 t2 t1
            beq t2 x0 -#8
            fence i o
            sb a0 t0 #0
            ret


$whatever
            csrrc a0 x0 #C01 ; time
            jal ra write_hex_u32
            addi a0 x0 #0D
            jal ra write
            addi a0 x0 #0A
            jal ra write

            jal x0 whatever

            inval
