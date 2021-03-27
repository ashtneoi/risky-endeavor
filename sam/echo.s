$read
            lui x1 #1000'0
            addi x2 x0 #01
            fence iorw iorw
$read_loop
            lb x4 x1 #5
            and x4 x4 x2
            beq x4 x0 read_loop
            fence iorw iorw
            lb x3 x1 #0

$write
            addi x2 x0 #20
            fence iorw iorw
$write_loop
            lb x4 x1 #5
            and x4 x4 x2
            beq x4 x0 write_loop
            fence iorw iorw
            sb x3 x1 #0
            jal x0 read

            inval
