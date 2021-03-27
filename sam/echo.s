$read
            lui x1 #1000'0
            addi x2 x0 #01
            fence iorw iorw
            lb x4 x1 #5
            and x4 x4 x2
            beq x4 x0 -#8
            fence iorw iorw
            lb x3 x1 #0

$write
            addi x2 x0 #20
            fence iorw iorw
            lb x4 x1 #5
            and x4 x4 x2
            beq x4 x0 -#8
            fence iorw iorw
            sb x3 x1 #0
            jal x0 read

            inval
