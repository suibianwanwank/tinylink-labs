#!/bin/sh
# 第 11 章开头的谜题与"边角"一节：GC 删掉函数之后，调试信息里填了什么。
# 在 Linux x86-64 主机上运行；工作目录 $W/tomb。
set -e
W=${W:-$HOME/.cache/linker-lab/ch11}
mkdir -p "$W/tomb" && cp "$(dirname "$0")"/gc.c "$(dirname "$0")"/dead.c "$W/tomb/" && cd "$W/tomb"
# -fdebug-prefix-map 把构建目录记成 /w，输出里的路径和偏移不随读者的目录变化
CF="-O1 -g -ffunction-sections -fno-pic -fno-asynchronous-unwind-tables -fdebug-prefix-map=$W/tomb=/w"
set -x
clang $CF -c gc.c -o gc_clang.o
gcc $CF -c gc.c -o gc_gcc.o
ld.lld --gc-sections gc_clang.o -o lld.out
ld.bfd --gc-sections gc_gcc.o -o bfd.out
llvm-dwarfdump --debug-info lld.out | grep -E 'DW_AT_(low_pc|name)'
llvm-dwarfdump --debug-addr lld.out
llvm-dwarfdump --debug-info bfd.out | grep -E 'DW_AT_(low_pc|name)'
llvm-dwarfdump --debug-line lld.out | grep -A15 '^Address'
llvm-dwarfdump --debug-line -v lld.out | sed -n '/line_base/,/opcode_base/p'
llvm-objdump -s -j .debug_line lld.out
llvm-readelf -rW gc_clang.o
# 合并后的 .debug_str：字符串顺序变了，.debug_str_offsets 跟着改
llvm-readelf -x .debug_str_offsets lld.out
llvm-objcopy --dump-section .debug_str=out_str.bin lld.out scratch.out
tr '\0' '\n' < out_str.bin | cat -n
# DWARF 4 的 .debug_ranges
gcc $CF -gdwarf-4 -c gc.c -o gc4.o
ld.lld --gc-sections gc4.o -o lld4.out
ld.bfd --gc-sections gc4.o -o bfd4.out
readelf -x .debug_ranges lld4.out
readelf -x .debug_ranges bfd4.out
# 代码从地址 0 开始时，墓碑 0 和真实地址撞车
ld.bfd --gc-sections -Ttext=0 gc_gcc.o -o bfd0.out
ld.lld --gc-sections --image-base=0 -Ttext=0 gc_gcc.o -o lld0.out
ld.lld --gc-sections --image-base=0 -Ttext=0 -z dead-reloc-in-nonalloc='.debug_*=0xffffffffffffffff' gc_gcc.o -o lldm1.out
nm bfd0.out
for f in bfd0.out lld0.out lldm1.out; do addr2line -f -e $f 0x4; llvm-symbolizer --obj=$f 0x4; done
llvm-dwarfdump --debug-line bfd0.out | grep -A20 '^Address'
# 本机 gas 默认不压缩；显式 -gz=zlib 创建独立压缩输入并验证两个链接器解压
readelf -SW gc_gcc.o
gcc $CF -gz=zlib -c gc.c -o gc_gcc_z.o
readelf -tW gc_gcc_z.o
readelf -rW gc_gcc_z.o
ld.bfd --gc-sections gc_gcc_z.o -o bfd_z.out
ld.lld --gc-sections gc_gcc_z.o -o lld_z.out
for f in bfd_z.out lld_z.out; do
    readelf -SW "$f"
    if readelf -SW "$f" | grep -E "\.debug_.*[[:space:]][A-Z]*C[A-Z]*[[:space:]]"; then exit 1; fi
    llvm-dwarfdump --verify "$f"
done
# 整个文件都被 GC 掉时，它的调试节去留
gcc $CF -c dead.c -o dead_gcc.o
ld.bfd --gc-sections --print-gc-sections gc_gcc.o dead_gcc.o -o bfd2.out
ld.lld --gc-sections gc_gcc.o dead_gcc.o -o lld2.out
llvm-dwarfdump --debug-info bfd2.out | grep -E 'DW_AT_name'
llvm-dwarfdump --debug-info lld2.out | grep -E 'DW_AT_name'
# build-id 的几种算法
for s in fast md5 sha1 uuid; do ld.lld --build-id=$s gc_clang.o -o bid_$s; llvm-readelf -n bid_$s | grep 'Build ID'; done
ld.bfd --build-id=fast gc_clang.o -o bid_bfd_fast
ld.bfd --build-id gc_clang.o -o bid_bfd; llvm-readelf -n bid_bfd | grep 'Build ID'
