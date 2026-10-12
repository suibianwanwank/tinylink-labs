# COMMON：4 字节，16 字节对齐。
        .comm   buf, 4, 16
        .text
        .globl  _start
_start: ret
