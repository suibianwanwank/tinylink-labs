        .section .vectors, "a"
        .balign 4
        .space 0x40
        .section .text, "ax"
        .balign 2
        .space 0x1a2
        .section .rodata, "a"
        .balign 8
        .space 0x31
        .section .data, "aw"
        .balign 8
        .space 0x26
        .section .bss, "aw", @nobits
        .balign 16
        .space 0x103
