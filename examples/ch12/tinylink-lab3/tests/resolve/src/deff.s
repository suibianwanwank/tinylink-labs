        .text
        .globl  f
f:      movl    $60, %eax
        movl    $42, %edi
        syscall
