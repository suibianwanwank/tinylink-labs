// toplevel.c：文件级的汇编里定义一个函数
__asm__(".text\n.globl asm_seven\n.type asm_seven,@function\nasm_seven:\n  movl $7, %eax\n  ret\n");
