# 先读到的 COMMON 更大：8 字节、8 字节对齐。
        .comm   buf, 8, 8
        .text
        .globl  _start
_start: ret
