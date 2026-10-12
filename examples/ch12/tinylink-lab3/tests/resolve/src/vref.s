        .globl  v
        .protected v
        .text
        .globl  _start
_start: movl    v(%rip), %eax
