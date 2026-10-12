# K 是绝对符号，值 42，不属于任何节。
        .globl  K
        .set    K, 42
        .text
        .globl  _start
_start: ret
