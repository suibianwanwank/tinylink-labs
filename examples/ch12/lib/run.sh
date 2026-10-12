#!/bin/sh
# 第 12 章：静态库里的 LTO 成员。
# 1) clang bitcode 打成库：GNU ar 与 llvm-ar 的索引不同，GNU ld 与 lld 反应不同。
# 2) GCC -flto 的 slim 对象打成库，交给 ld.lld：undefined symbol。
set -e
. "$(dirname "$0")/../env.sh"
mkdir -p "$W/lib" && cp "$(dirname "$0")"/*.c "$W/lib/" && cd "$W/lib"
rm -f *.a
set -x
for f in app mul3 other; do $CC -O2 -flto -c $f.c -o $f.o; done
ar rcs libm3_gnu.a mul3.o other.o
llvm-ar rcs libm3_llvm.a mul3.o other.o
nm -s libm3_gnu.a || true
llvm-nm --print-armap libm3_llvm.a | head -3
link_musl app_gnu app.o -L. -lm3_gnu
link_musl app_llvm app.o -L. -lm3_llvm --save-temps
cat app_llvm.resolution.txt
llvm-nm app_llvm | grep -E ' (main|mul3|other)$'
gcc -O2 -c app.c -o app_gcc.o
gcc -static app_gcc.o -L. -lm3_gnu -o g1 || true
gcc -static app_gcc.o -L. -lm3_llvm -o g2 || true
# 库作者用 GCC 的 -flto 编库，使用者用 ld.lld 链接
for f in mul3 other; do gcc -O2 -flto -c $f.c -o $f.gcc.o; done
ar rcs libm3_slim.a mul3.gcc.o other.gcc.o
nm -s libm3_slim.a | head -4
readelf -sW mul3.gcc.o | tail -2
link_musl app_slim app.o -L. -lm3_slim || true
# 修法一：fat 对象
for f in mul3 other; do gcc -O2 -flto -ffat-lto-objects -c $f.c -o $f.fat.o; done
ar rcs libm3_fat.a mul3.fat.o other.fat.o
link_musl app_fat app.o -L. -lm3_fat && llvm-nm app_fat | grep -E ' (main|mul3)$'
# 修法二：同一套工具链，GCC 驱动 GNU ld 和 liblto_plugin
gcc -O2 -flto -c app.c -o app.gcc.o
gcc -O2 -flto -static app.gcc.o -L. -lm3_slim -o g_slim && nm g_slim | grep -E ' (main|mul3)$'
run_x86 sh -c 'cd lib; ./app_llvm a b; echo app_llvm=$?; ./app_fat a b; echo app_fat=$?; ./g_slim a b; echo g_slim=$?'
