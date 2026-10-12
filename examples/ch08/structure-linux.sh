#!/bin/sh
set -eu
[ "$(uname -s):$(uname -m)" = Linux:x86_64 ] || exit 2
D=$(cd "$(dirname "$0")" && pwd)
W=$(mktemp -d /tmp/linker-ch08-structure.XXXXXX)
cp "$D"/leaf.c "$D"/hand.s "$D"/gc.c "$D"/thrower.cc "$W/"
cd "$W"; printf 'work: %s\n' "$W"
set -x
clang -O1 -S leaf.c
clang -O1 -c leaf.c -o leaf.o
clang -O1 -g -fno-asynchronous-unwind-tables -c leaf.c -o dbg.o
clang -c hand.s -o hand.o
clang++ -O1 -c thrower.cc -o thrower.o
for f in leaf.o dbg.o hand.o thrower.o; do
 readelf -SW "$f"
 objdump --dwarf=frames "$f"
 objdump -dr "$f"
 objdump -s -j .eh_frame "$f" 2>/dev/null || true
done
objdump -s -j .gcc_except_table thrower.o
clang -O1 -ffreestanding -fno-stack-protector -fasynchronous-unwind-tables -ffunction-sections -c gc.c -o gc.o
ld.lld -static --eh-frame-hdr --gc-sections -e _start gc.o -o lldgc
ld.bfd -static --eh-frame-hdr --gc-sections -e _start gc.o -o bfdgc
for f in lldgc bfdgc; do
 readelf -SW "$f"; nm -n "$f"; objdump -s -j .eh_frame_hdr -j .eh_frame "$f"; readelf --debug-dump=frames "$f"
done
cat leaf.s
gcc -O1 -S leaf.c -o leaf-gcc.s
cat leaf-gcc.s
