# exit42.s 的 AArch64 版本，用来在原生 arm64 容器里让真实内核加载 tinylink 的产物。
        .text
        .globl  _start
        .type   _start,%function
_start:
        mov     x0, #42         // 第一个参数：退出码
        mov     x8, #93         // 系统调用号 93：exit
        svc     #0
        .size   _start, . - _start
        .section .note.GNU-stack,"",%progbits
