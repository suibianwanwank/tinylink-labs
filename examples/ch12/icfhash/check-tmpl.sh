#!/bin/sh
# 用一个 C++ 模板程序对比 icfhash 与 ld.lld --print-icf-sections。
# 只比 tmpl.o 里以 .text. 开头的节，比较排序后的（保留节，被删节）对。
set -e
. "$(dirname "$0")/../env.sh"
HERE=$(cd "$(dirname "$0")" && pwd)
[ -d "$HERE/../tinylink-lab3" ] || sh "$HERE/fetch-tinylink.sh"
(cd "$HERE" && cargo build --release -q)
BIN=$HERE/target/release/icfhash
mkdir -p "$W/tmpl" && cp "$HERE/tmpl.cpp" "$W/tmpl/" && cd "$W/tmpl"
clang++ -O2 -fno-pie -ffunction-sections -c tmpl.cpp -o tmpl.o
linkcxx() { out=$1; shift; clang++ -fuse-ld=lld -static -no-pie tmpl.o -o "$out" $(printf '%s\n' "$@" | sed '/^$/d;s/^/-Wl,/'); }
linkcxx t_none
for mode in all safe; do
  opt=""; [ $mode = safe ] && opt=--safe
  "$BIN" $opt tmpl.o 2>"icfhash-$mode.txt" | sh "$HERE/pairs.sh" tmpl.o > "mine-$mode.txt"
  linkcxx t_$mode --icf=$mode --print-icf-sections > "lld-$mode.log" 2>&1
  sh "$HERE/pairs.sh" tmpl.o < "lld-$mode.log" > "lld-$mode.txt"
  echo "== $mode: $(cat icfhash-$mode.txt)"
  echo "icfhash $(wc -l < mine-$mode.txt | tr -d ' ') 对，lld $(wc -l < lld-$mode.txt | tr -d ' ') 对"
  diff "mine-$mode.txt" "lld-$mode.txt" && echo "两边一致"
  echo "lld 在 tmpl.o 里折叠的非代码节：$(grep 'removing identical section tmpl.o:(' lld-$mode.log | grep -vc ':(.text')"
  grep 'removing identical section tmpl.o:(' lld-$mode.log | grep -v ':(.text' | sed 's/^ *removing identical section tmpl.o:(//; s/)$//' | awk -F. '{print "." $2}' | sort | uniq -c
  echo "lld 在 libstdc++.a 成员里折叠：$(grep 'removing identical section' lld-$mode.log | grep -c 'libstdc++')"
done
llvm-size t_none t_safe t_all
run_x86 sh -c 'cd tmpl; for p in t_none t_safe t_all; do ./$p; echo "$p=$?"; done'
