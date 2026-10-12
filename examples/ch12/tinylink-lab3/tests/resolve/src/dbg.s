# 唯一用到 u 的重定位在 .debug_info 里，这个节不可分配。
        .text
        .globl  _start
_start: ret
        .section .debug_info,"",@progbits
        .quad   u
