# fast_add_cet.s：补上 endbr64、.note.gnu.property 和 .note.GNU-stack
    .text
    .globl fast_add
    .type fast_add, @function
fast_add:
    endbr64
    leal (%rdi,%rsi), %eax
    ret
    .size fast_add, .-fast_add

    .section .note.gnu.property,"a",@note
    .p2align 3
    .long 4                 # n_namesz：名字 "GNU\0" 长 4
    .long 16                # n_descsz：一条属性，8 字节对齐后 16 字节
    .long 5                 # n_type：NT_GNU_PROPERTY_TYPE_0
    .asciz "GNU"
    .long 0xc0000002        # GNU_PROPERTY_X86_FEATURE_1_AND
    .long 4                 # pr_datasz
    .long 3                 # IBT | SHSTK
    .p2align 3              # pr_data 补齐到 8 字节

    .section .note.GNU-stack,"",@progbits
