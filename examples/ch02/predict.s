# predict.s
        .text
        .globl  entry
        .type   entry,@function
entry:
        movl    counter(%rip), %eax
        call    bump
        call    ext_func
        leaq    .Lmsg(%rip), %rdi
        jmp     .Lout
.Lout:
        ret
        .size   entry, . - entry

        .section .text.cold,"ax",@progbits
bump:
        ret

        .data
        .globl  counter
counter:
        .long   7
        .long   .Lout - entry
slot:
        .quad   counter + 8
        .quad   slot

        .section .rodata.str1.1,"aMS",@progbits,1
.Lmsg:
        .asciz  "ok"
        .section .note.GNU-stack,"",@progbits
