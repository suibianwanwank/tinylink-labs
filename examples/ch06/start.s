    .globl _start
_start:
    xor %ebp, %ebp
    and $-16, %rsp
#ifdef MISALIGN
    sub $8, %rsp
#endif
    call start_c
