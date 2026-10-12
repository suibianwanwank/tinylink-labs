.text
.globl answer
.type answer, @function
answer:
    mov $42, %eax
    ret
.size answer, .-answer
.section .note.GNU-stack,"",@progbits
