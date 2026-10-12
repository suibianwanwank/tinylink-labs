# 实验 2 的反例：退出码从 .data 里读，movl 的 RIP 相对寻址需要一条重定位。
        .text
        .globl  _start
_start:
        movl    $60, %eax
        movl    code(%rip), %edi
        syscall
        .data
code:
        .long   42
        .section .note.GNU-stack,"",@progbits
