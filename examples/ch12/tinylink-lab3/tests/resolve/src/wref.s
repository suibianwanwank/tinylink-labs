# 对 w 的弱引用。
        .weak   w
        .text
        .globl  _start
_start: call    w
