        .section .boot,"ax",@progbits
        .globl  _start
_start:
        movl    $60, %eax
        movl    $42, %edi
        syscall
        .section .note.GNU-stack,"",@progbits
