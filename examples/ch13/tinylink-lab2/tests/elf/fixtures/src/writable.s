# 实验 2 的反例：有一个可写的 .data 节，而 M2 只输出一个只读、可执行的段。
        .text
        .globl  _start
_start:
        movl    $60, %eax
        movl    $42, %edi
        syscall
        .data
        .long   42
        .section .note.GNU-stack,"",@progbits
