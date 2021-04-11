$read
            lui x1 #1000'0
            addi x2 x0 #01
            lb x4 x1 #5
            and x4 x4 x2
            beq x4 x0 -#8
            lb x3 x1 #0

$write
            addi x2 x0 #20
            lb x4 x1 #5
            and x4 x4 x2
            beq x4 x0 -#8
            sb x3 x1 #0
            jal x0 read

            inval
