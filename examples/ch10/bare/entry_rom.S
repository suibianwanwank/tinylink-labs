        .section .text.entry, "ax"
        .globl _entry
_entry:
        la   sp, stack_top
        # 把 .data 的初值从 LMA（ROM）拷到 VMA（RAM）
        la   t0, _sidata
        la   t1, _sdata
        la   t2, _edata
1:      bgeu t1, t2, 2f
        ld   t3, 0(t0)
        sd   t3, 0(t1)
        addi t0, t0, 8
        addi t1, t1, 8
        j    1b
        # 把 .bss 清零
2:      la   t1, _sbss
        la   t2, _ebss
3:      bgeu t1, t2, 4f
        sd   zero, 0(t1)
        addi t1, t1, 8
        j    3b
4:      call main
spin:   wfi
        j    spin
