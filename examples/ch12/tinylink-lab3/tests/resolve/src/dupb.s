# f 在这里是 .data 里的一个字：名字相同，链接器不看它是函数还是数据。
        .data
        .long   0
        .globl  f
f:      .long   1
