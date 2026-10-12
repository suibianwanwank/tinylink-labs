# h 是 hidden 的强引用，有重定位用到它，没有定义。
        .globl  h
        .hidden h
        .text
        .globl  _start
_start: call    h
