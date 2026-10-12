#!/bin/sh
# 用 Linux x86-64 的 GDB 查看 GC 墓碑对应的源码地址。
set -eu
W=${W:-$HOME/.cache/linker-lab/ch11}
cd "$W/tomb"
for f in bfd.out bfd0.out lldm1.out; do
 echo "== $f"
 gdb -q -batch -ex "info line used" -ex "info line unused" -ex "info line *0x4" -ex "break used" ./$f 2>&1
done
