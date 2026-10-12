#!/bin/sh
# Linux x86-64 GCC / GNU gold 对照。
set -x
ld.gold --version | head -1
gcc -O2 -ffunction-sections -c icf.c -o icf_gcc.o
gcc -O2 -ffunction-sections -c ptr.c -o ptr_gcc.o
gcc -fuse-ld=gold -Wl,--icf=all -Wl,--print-icf-sections icf_gcc.o ptr_gcc.o -o gptr_all
gcc -fuse-ld=gold -Wl,--icf=safe -Wl,--print-icf-sections icf_gcc.o ptr_gcc.o -o gptr_safe
nm gptr_all | grep -E ' (add_i|add_j)$'
nm gptr_safe | grep -E ' (add_i|add_j)$'
./gptr_all; echo "gptr_all=$?"
./gptr_safe; echo "gptr_safe=$?"
gcc -fuse-ld=bfd -Wl,--icf=all icf_gcc.o ptr_gcc.o -o gptr_bfd
# GCC 的 LTO 交给 gold：同一个 liblto_plugin.so
gcc -O2 -flto -c ../puzzle/main.c -o m.o && gcc -O2 -flto -c ../puzzle/util.c -o u.o
gcc -O2 -flto -fuse-ld=gold m.o u.o -o g_lto -v 2>&1 | grep -oE 'collect2 .*-plugin [^ ]+' | cut -c1-200
nm g_lto | grep -E ' (main|scale|twice|checksum)$'
ls /usr/lib/bfd-plugins/
