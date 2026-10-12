# 对 hook 的弱引用，并且有一条调用它的重定位。
        .weak   hook
        .text
        .globl  _start
_start: call    hook
