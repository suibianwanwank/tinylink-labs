# 先读到的是 hidden 的引用，后读到的定义是 default：结果仍是 hidden。
        .globl  v
        .hidden v
        .text
        .globl  _start
_start: movl    v(%rip), %eax
