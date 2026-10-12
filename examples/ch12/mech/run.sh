#!/bin/sh
# 第 12 章"bitcode""GCC 的 LTO 对象""链接器在 LTO 里做什么""插件接口""看中间产物"几节。
# 先运行 ../puzzle/run.sh。
set -e
. "$(dirname "$0")/../env.sh"
cd "$W/puzzle"
set -x
file main.o main.lto.o
xxd main.lto.o | head -1
readelf -h main.lto.o || true
ls -l main.o main.lto.o
llvm-dis util.lto.o -o - | grep -A4 '@scale'
llvm-bcanalyzer -dump main.lto.o | grep -oE '^ *<[A-Z_]+BLOCK' | sort | uniq -c
llvm-bcanalyzer -dump util.lto.o | grep -A1 -m1 IDENTIFICATION_BLOCK
llvm-nm main.lto.o util.lto.o
# lld 的中间产物与解析结果
mkdir -p st && find st -mindepth 1 -delete && cp main.lto.o util.lto.o st/ && cd st
link_musl lto main.lto.o util.lto.o --save-temps
ls
cat lto.resolution.txt
for f in lto.0.0.preopt.bc lto.0.2.internalize.bc lto.0.4.opt.bc; do llvm-dis $f -o - | grep -E '^define'; done
file lto.lto.o
cd .. && mkdir -p st2 && find st2 -mindepth 1 -delete && cd st2
link_musl lto ../main.lto.o ../util.lto.o -plugin-opt=save-temps -Map=lto.map
link_musl plain ../main.o ../util.o -Map=plain.map
ls
grep -E 'main\.o:\(\.text|lto\.lto\.o:\(\.text\.main' plain.map lto.map
# GCC：.gnu.lto_ 节、slim 与 fat、插件
mkdir -p "$W/gcc" && cd "$W/gcc" && cp ../puzzle/main.c ../puzzle/util.c .
gcc -O2 -flto -c util.c -o util.slim.o
gcc -O2 -flto -ffat-lto-objects -c util.c -o util.fat.o
gcc -O2 -c util.c -o util.o
ls -l util.*o
readelf -SW util.slim.o
readelf -sW util.slim.o | tail -1
readelf -SW util.fat.o | grep ' .text'
nm util.slim.o
nm --plugin /dev/null util.slim.o || true
for i in 1 2; do gcc -O2 -flto -c util.c -o t$i.o; readelf -SW t$i.o | grep -m1 -oE 'lto_scale[^ ]+'; done
for i in 1 2; do gcc -O2 -flto -frandom-seed=x -c util.c -o r$i.o; done
cmp r1.o r2.o && echo '固定 seed 后逐字节相同'
gcc -O2 -flto -c main.c -o main.slim.o
gcc -O2 -flto -static main.slim.o util.slim.o -o g_lto -v 2>&1 | grep -oE 'collect2 .*-plugin [^ ]+|-plugin-opt=[^ ]+' | head -6
file "$(gcc -print-prog-name=liblto_plugin.so)"
nm g_lto | grep -E ' (main|scale|twice|checksum)$'
mkdir -p st && find st -mindepth 1 -delete && cd st && cp ../main.c ../util.c .
gcc -O2 -flto -c main.c util.c
gcc -O2 -flto -static -save-temps main.o util.o -o g
ls
cat g.res
# clang 把 -mcpu 之类的参数以 -plugin-opt= 交给链接器
clang -flto -fuse-ld=lld -march=x86-64-v3 -O2 ../../puzzle/main.lto.o ../../puzzle/util.lto.o -### 2>&1 | grep -oE '"-plugin-opt=[^"]+"'
