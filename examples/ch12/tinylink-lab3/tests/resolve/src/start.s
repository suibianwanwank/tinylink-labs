# 只有 _start：退出码 42。
        .text
        .globl  _start
_start: movl    $60, %eax
        movl    $42, %edi
        syscall
