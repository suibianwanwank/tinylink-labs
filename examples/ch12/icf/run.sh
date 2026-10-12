#!/bin/sh
# 第 12 章 ICF：--icf=safe 与 --icf=all、函数指针相等、"比字节不够"。
set -e
. "$(dirname "$0")/../env.sh"
mkdir -p "$W/icf" && cp "$(dirname "$0")"/*.c "$(dirname "$0")"/gold.sh "$W/icf/" && cd "$W/icf"
set -x
$CC -O2 -ffunction-sections -c icf.c -o icf.o
llvm-objdump -d icf.o
llvm-objdump -s -j .llvm_addrsig icf.o
readelf -sW icf.o
readelf -SW icf.o | grep -E 'addrsig'
ld.lld -e pick --icf=safe --print-icf-sections icf.o -o icf_safe
nm icf_safe | grep -E 'add_|pick'
ld.lld -e pick --icf=all --print-icf-sections icf.o -o icf_all
nm icf_all | grep -E 'add_|pick'
# 能运行的版本：比较两个函数指针
$CC -O2 -ffunction-sections -c ptr.c -o ptr.o
link_musl ptr_none icf.o ptr.o
link_musl ptr_safe icf.o ptr.o --icf=safe
link_musl ptr_all icf.o ptr.o --icf=all
# 去掉 .llvm_addrsig 以后，safe 怎么办
$CC -O2 -ffunction-sections -fno-addrsig -c icf.c -o icf_noaddrsig.o
ld.lld -e pick --icf=safe --print-icf-sections icf_noaddrsig.o -o icf_safe2
# 字节相同、调用对象不同
$CC -O2 -ffunction-sections -fno-inline -c chain.c -o chain.o
llvm-objdump -d --no-show-raw-insn chain.o | grep -A3 '<f[123]>:'
llvm-readelf -r chain.o | grep -E 'Relocation section|g[123]'
link_musl chain chain.o --icf=all --print-icf-sections 2>&1 | grep -A1 "chain.o:(.text.[fg]"
llvm-nm chain | grep -E ' [fg][123]$' | sort
run_x86 sh -c 'cd icf; for p in ptr_none ptr_safe ptr_all chain; do ./$p; echo "$p=$?"; done'
# 调试信息：被折叠的 add_j 在 .debug_info 与 .debug_line 里指向哪里
$CC -O2 -g -ffunction-sections -c icf.c -o icf_g.o
ld.lld -e pick --icf=all icf_g.o -o icf_g_all
llvm-dwarfdump --debug-info icf_g_all | grep -E 'DW_AT_(name|low_pc)'
llvm-dwarfdump --debug-addr icf_g_all
llvm-dwarfdump --debug-line icf_g_all | sed -n '/^Address/,$p'
llvm-symbolizer --obj=icf_g_all 0x201170
# gold 与 GNU ld（BFD）：在本机 Linux x86-64 上
(cd "$W/icf"; sh gold.sh)
