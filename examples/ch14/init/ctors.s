# ctors.s：一个 .ctors.00100 和一个 .init_array.00101，看 GNU ld 和 lld 怎样放
.section .ctors.00100,"aw"
.quad 0
.section .init_array.00101,"aw",@init_array
.quad 0
.text
.globl _start
_start: ret
