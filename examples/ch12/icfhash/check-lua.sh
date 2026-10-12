#!/bin/sh
# 用 Lua 5.4.7 的 33 个目标文件（-ffunction-sections）对比 icfhash 与 ld.lld --print-icf-sections。
# 先运行 ../thinlto/build.sh 下载并解开 Lua 源码。
set -e
. "$(dirname "$0")/../env.sh"
HERE=$(cd "$(dirname "$0")" && pwd)
[ -d "$HERE/../tinylink-lab3" ] || sh "$HERE/fetch-tinylink.sh"
(cd "$HERE" && cargo build --release -q)
BIN=$HERE/target/release/icfhash
cd "$W/src/lua-5.4.7"
mkdir -p obj-fs && find obj-fs -mindepth 1 -delete
SYSINC="-nostdinc -isystem /usr/include/x86_64-linux-musl"
for f in $(ls src/*.c | grep -v luac.c); do
  b=$(basename "$f" .c); $CC $SYSINC -O2 -DLUA_USE_POSIX -ffunction-sections -c "$f" -o "obj-fs/$b.o"
done
for mode in all safe; do
  opt=""; [ $mode = safe ] && opt=--safe
  "$BIN" $opt obj-fs/*.o 2>"icfhash-$mode.txt" | sh "$HERE/pairs.sh" obj-fs/ > "mine-$mode.txt"
  link_musl lua-icf-$mode obj-fs/*.o --icf=$mode --print-icf-sections 2>&1 | sh "$HERE/pairs.sh" obj-fs/ > "lld-$mode.txt"
  echo "== $mode: $(cat icfhash-$mode.txt)"
  echo "icfhash $(wc -l < mine-$mode.txt | tr -d ' ') 对，lld $(wc -l < lld-$mode.txt | tr -d ' ') 对"
  diff "mine-$mode.txt" "lld-$mode.txt" && echo "两边一致"
done
