# 声明了一个全局的 missing，却没有任何指令用到它：不算未定义错误。
        .globl  missing
        .text
        .globl  _start
_start: ret
