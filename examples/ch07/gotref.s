	.section .note.GNU-stack,"",@progbits
	.text
	.globl gotbase
gotbase:
	leaq _GLOBAL_OFFSET_TABLE_(%rip), %rax
	ret
