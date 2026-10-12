# Supplied freestanding startup. The linker fills the descriptor at byte 128.
# Only local RIP-relative operands occur here; there are no ELF relocations.
.section .text,"ax",@progbits
.Lanchor:
    lea .Lanchor(%rip), %r11
    sub .Ldescriptor(%rip), %r11
    mov .Ldescriptor+8(%rip), %rsi
    add %r11, %rsi
    mov .Ldescriptor+16(%rip), %rcx
    test %rcx, %rcx
    jz .Lenter
.Lrelocate:
    mov (%rsi), %rdi
    add %r11, %rdi
    mov 16(%rsi), %rax
    add %r11, %rax
    mov %rax, (%rdi)
    add $24, %rsi
    dec %rcx
    jnz .Lrelocate
.Lenter:
    mov .Ldescriptor+24(%rip), %rax
    add %r11, %rax
    xor %edx, %edx
    jmp *%rax
.org 128, 0
.Ldescriptor:
    .quad 0 # Startup RVA, used to recover the load bias.
    .quad 0 # RELA table RVA.
    .quad 0 # Number of 24-byte RELA records.
    .quad 0 # User entry RVA.
.section .note.GNU-stack,"",@progbits
