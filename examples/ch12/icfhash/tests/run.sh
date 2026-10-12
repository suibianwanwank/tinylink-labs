#!/bin/sh
# icfhash 的两个回归测试：弱定义（按第 3 章选强定义）与 COMDAT 去重。和 ld.lld 对拍。
set -e
. "$(dirname "$0")/../../env.sh"
HERE=$(cd "$(dirname "$0")" && pwd)
BIN=$HERE/../target/release/icfhash
mkdir -p "$W/icfhash-tests" && cp "$HERE"/*.c "$HERE"/*.cpp "$W/icfhash-tests/" && cd "$W/icfhash-tests"
for f in weak_a weak_b; do $CC -O2 -ffunction-sections -c $f.c -o $f.o; done
for f in comdat_a comdat_b; do clang++ --target=x86_64-unknown-linux-gnu -O2 -ffunction-sections -fno-exceptions -c $f.cpp -o $f.o; done
llvm-readelf -sW weak_a.o weak_b.o | grep -E ' g$'
llvm-readelf -SW comdat_a.o comdat_b.o | grep -E 'GROUP|_Z5twice'
for t in "weak_a.o weak_b.o" "comdat_a.o comdat_b.o"; do
  echo "== $t"
  "$BIN" $t 2>/dev/null | sh "$HERE/../pairs.sh" "" > mine.txt
  link_musl out $t --icf=all --print-icf-sections 2>&1 | grep -v libc.a | grep -v crt | sh "$HERE/../pairs.sh" "" > lld.txt
  cat mine.txt; diff mine.txt lld.txt && echo "两边一致"
done
