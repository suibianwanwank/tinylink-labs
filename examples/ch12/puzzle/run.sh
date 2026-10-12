#!/bin/sh
# 第 12 章开头的谜题：两个文件，不开 LTO 与开 -flto 的对比。
# 同一台 Linux 上分别使用 Clang/LLD 与 GCC，直接执行生成的 ELF。
set -e
. "$(dirname "$0")/../env.sh"
mkdir -p "$W/puzzle" && cp "$(dirname "$0")"/main.c "$(dirname "$0")"/util.c "$W/puzzle/" && cd "$W/puzzle"
set -x
$CC -O2 -c main.c -o main.o
$CC -O2 -c util.c -o util.o
link_musl plain main.o util.o
$CC -O2 -flto -c main.c -o main.lto.o
$CC -O2 -flto -c util.c -o util.lto.o
link_musl lto main.lto.o util.lto.o
llvm-nm -S plain | grep -E ' (main|scale|twice|checksum)$'
llvm-nm -S lto | grep -E ' (main|scale|twice|checksum)$'
llvm-size plain lto
llvm-objdump -d --no-show-raw-insn --disassemble-symbols=main plain lto
run_x86 sh -c 'for p in plain lto plain lto; do /usr/bin/time -f "$p %e s" ./puzzle/$p; done; true'
bash -c '
  gcc -O2 main.c util.c -o a_plain && gcc -O2 -flto main.c util.c -o a_lto
  TIMEFORMAT="%R s"
  for p in a_plain a_lto a_plain a_lto a_plain a_lto; do { time ./$p; } 2>&1 | sed "s/^/$p /"; done
  size a_plain a_lto
  nm -S a_lto | grep -E " (main|scale|twice|checksum)$"'
