// odd.s：用 x19 当帧基址，compact unwind 的编码表达不了
    .text
    .globl _odd
    .p2align 2
_odd:
    .cfi_startproc
    stp  x19, x30, [sp, #-16]!
    .cfi_def_cfa_offset 16
    .cfi_offset w30, -8
    .cfi_offset w19, -16
    mov  x19, sp
    .cfi_def_cfa w19, 16
    mov  w0, #7
    ldp  x19, x30, [sp], #16
    .cfi_def_cfa wsp, 0
    .cfi_restore w19
    .cfi_restore w30
    ret
    .cfi_endproc
