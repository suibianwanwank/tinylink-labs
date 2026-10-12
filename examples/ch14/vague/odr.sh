#!/bin/sh
# ODR 违规：两个翻译单元里的 Greeter 虚函数顺序不同，链接顺序决定谁的 vtable 留下
. "$(dirname "$0")/../env.sh"; stage odr
set -x
for f in odr_a odr_b odr_main; do $GXX -O0 -c $f.cpp; done
$NM -C odr_a.o | grep -v ' U '
$RE -gW odr_a.o | grep -E "group section"
# 虚表和 typeinfo 的布局
$RE -SW odr_a.o | grep -E '_ZT[VIS]7Greeter'
$RE -rW odr_a.o | sed -n '/rela.rodata._ZTV7Greeter/,/^$/p;/rela.rodata._ZTI7Greeter/,/^$/p'
$OD -s -j .rodata._ZTS7Greeter odr_a.o | tail -1
$OD -dr --no-show-raw-insn -j .text._ZN7GreeterC2Ev odr_a.o | grep -A1 'mov    \$0x0,%edx'
$OD -d --no-show-raw-insn odr_a.o | sed -n '/<_Z6call_aP7Greeter>:/,/ret/p' | grep -E '\(%rax\)|add|call'
$OD -d --no-show-raw-insn odr_b.o | sed -n '/<_Z6call_bP7Greeter>:/,/ret/p' | grep -E '\(%rax\)|add|call' 
ok $GXX -static odr_main.o odr_a.o odr_b.o -o ab && run_x86 ./ab
ok $GXX -static odr_main.o odr_b.o odr_a.o -o ba && run_x86 ./ba
$NM ab | grep -E "_ZN7Greeter|_ZTV7"
exit $FAIL
