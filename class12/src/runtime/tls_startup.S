# Supplied static-TLS startup. The linker fills the descriptor at byte 128.
# Class9's relocation startup jumps here after applying RELATIVE records, so
# the template already holds load-time addresses when it is copied. Only local
# RIP-relative operands occur; there are no ELF relocations. The stack pointer
# is never changed, and %rdx is cleared before entering user code.
.section .text,"ax",@progbits
.Lanchor:
    lea .Lanchor(%rip), %r8
    sub .Ldescriptor(%rip), %r8          # r8 = load bias
    mov .Ldescriptor+16(%rip), %rdi
    add %r8, %rdi                        # rdi = TLS area (zero-filled BSS)
    mov .Ldescriptor+8(%rip), %rsi
    add %r8, %rsi                        # rsi = initialization image
    mov .Ldescriptor+24(%rip), %rcx      # rcx = image size
    cld
    rep movsb
    mov .Ldescriptor+16(%rip), %rsi
    add %r8, %rsi
    add .Ldescriptor+32(%rip), %rsi      # rsi = thread pointer
    mov %rsi, (%rsi)                     # TCB word 0 points to itself
    mov $0x1002, %edi                    # ARCH_SET_FS
    mov $158, %eax                       # arch_prctl; clobbers rcx and r11
    syscall
    test %rax, %rax
    jnz .Lfail
    mov %r8, %rax                        # syscall preserves r8
    add .Ldescriptor+40(%rip), %rax      # user entry
    xor %edx, %edx
    jmp *%rax
.Lfail:
    mov $127, %edi
    mov $231, %eax                       # exit_group
    syscall
    hlt
.org 128, 0
.Ldescriptor:
    .quad 0 # Startup RVA, used to recover the load bias.
    .quad 0 # Initialization image RVA.
    .quad 0 # TLS area RVA.
    .quad 0 # Initialization image size.
    .quad 0 # Thread-pointer offset within the TLS area.
    .quad 0 # User entry RVA.
.section .note.GNU-stack,"",@progbits
