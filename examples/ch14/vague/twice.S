# twice.S：手写 twice(int) 的一份"头文件实例"，用宏选择绑定和是否进 COMDAT 组
# 用法：clang -c -DFILE=1 -DWEAK -DCOMDAT twice.S
#if defined(COMDAT)
        .section .text._Z5twicei,"axG",@progbits,_Z5twicei,comdat
#else
        .section .text._Z5twicei,"ax",@progbits
#endif
#if defined(WEAK)
        .weak   _Z5twicei
#else
        .globl  _Z5twicei
#endif
        .type   _Z5twicei,@function
_Z5twicei:
        leal    (%rdi,%rdi), %eax
        ret
        .size   _Z5twicei, .-_Z5twicei

#define CAT(a, b) a##b
#define USER(n) CAT(user, n)
        .text
        .globl  USER(FILE)
USER(FILE):
        call    _Z5twicei
        ret
