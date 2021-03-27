            lui sp #8000'1

            lui a0 #AB90'4
            addi a0 a0 #321
            jal ra write_hex_u32

            inval


$write_hex_u32
            addi sp sp #8
            sw ra sp -#8
            sw s0 sp -#4

            addi s0 a0 #0

            srli a0 s0 #10
            jal ra write_hex_u16
            addi a0 x0 #60
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


$write
            lui t0 #1000'0
            addi t1 x0 #20
            fence iorw iorw
            lb t2 t0 #5
            and t2 t2 t1
            beq t2 x0 -#8
            fence iorw iorw
            sb a0 t0 #0
            ret
