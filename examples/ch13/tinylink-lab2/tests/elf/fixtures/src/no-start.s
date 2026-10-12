# 实验 2 的反例：没有 _start，链接器不知道入口在哪里。
        .text
        .globl  begin
begin:
        movl    $60, %eax
        movl    $42, %edi
        syscall
        .section .note.GNU-stack,"",@progbits
