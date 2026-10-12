# start.S：没有 C 库时的程序入口
        .text
        .globl  _start
_start:
        call    main
        movl    %eax, %edi      # main 的返回值作为退出码
        movl    $60, %eax       # 60 是 x86-64 Linux 的 exit 系统调用号
        syscall
        .section .note.GNU-stack,"",@progbits
