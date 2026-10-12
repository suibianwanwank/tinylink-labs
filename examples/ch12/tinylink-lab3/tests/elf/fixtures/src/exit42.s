# 实验 2 的 M2 输入：不用 C 库，直接用 exit 系统调用以退出码 42 结束，没有重定位。
        .text
        .globl  _start
        .type   _start,@function
_start:
        movl    $60, %eax       # 系统调用号 60：exit
        movl    $42, %edi       # 第一个参数：退出码
        syscall
        .size   _start, . - _start
        .section .note.GNU-stack,"",@progbits
