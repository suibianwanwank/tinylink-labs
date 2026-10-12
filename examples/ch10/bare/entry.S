        .section .text.entry, "ax"
        .globl _entry
_entry:
        la   sp, stack_top      # 栈顶由链接器脚本给出
        call main
spin:   wfi
        j    spin
