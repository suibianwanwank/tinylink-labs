# h 是 hidden 的强引用，既没有定义，也没有重定位用到它。
        .globl  h
        .hidden h
        .text
        .globl  _start
_start: ret
