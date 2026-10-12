# 另一个输入里的只读数据，没有人引用它。
        .section .rodata,"a",@progbits
        .globl  banner
banner: .ascii  "tinylink lab3"
