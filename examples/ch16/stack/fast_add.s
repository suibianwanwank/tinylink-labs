    .text
    .globl fast_add
    .type fast_add, @function
fast_add:
    leal (%rdi,%rsi), %eax
    ret
    .size fast_add, .-fast_add
