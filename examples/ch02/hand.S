# hand.S：手写一个目标文件
        .section .text,"ax",@progbits
        .globl  add
        .type   add,@function
add:
        leal    (%rdi,%rsi), %eax
        ret
        .size   add, . - add

        .section .data,"aw",@progbits
        .balign 4
        .globl  answer
        .type   answer,@object
answer:
        .long   42
        .size   answer, 4

        .section .rodata,"a",@progbits
greeting:
        .asciz  "hi"
        .byte   0xff
        .balign 8
        .uleb128 624485
        .sleb128 -123456

# 宏：\name 和 \value 是参数，展开时被替换
        .macro  CONST name, value
        .globl  \name
\name:
        .long   \value
        .endm

#ifdef BIG
        CONST   limit, 100000
#else
        CONST   limit, 100
#endif

        .section .note.GNU-stack,"",@progbits
