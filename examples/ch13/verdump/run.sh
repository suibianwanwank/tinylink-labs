#!/bin/sh
# 第 13 章练习四：verdump。在 x86-64 Linux 上用 cargo 构建，读取同机生成的 ELF，
# 输出和 GNU readelf 的 --dyn-syms -W 名字列逐行比较。先运行 ../symver/run.sh 和 ../glibc/run.sh。
set -e
. "$(dirname "$0")/../env.sh"
SRC=$(cd "$(dirname "$0")" && pwd)
for d in "$W/tinylink-lab2" "$W/verdump"; do mkdir -p "$d" && find "$d" -mindepth 1 -delete; done
# 固定的 lab2 源码随本章示例一起分发，避免依赖作者的私人路径。
cp -R "$SRC/../tinylink-lab2"/. "$W/tinylink-lab2/"
mkdir -p "$W/verdump" && cp -R "$SRC/Cargo.toml" "$SRC/src" "$W/verdump/"
(cd "$W/verdump" && cargo build -q --release)
V=$W/verdump/target/release/verdump
RE=readelf     # 本轮原生参考为 GNU readelf 2.46
$V "$W/symver/v2/libparse.so.1"
$V "$W/symver/app_new" | grep parse
# 对拍：取 readelf 的 "Num:" 和 Name 列（未定义符号带 " (n)"）
for f in symver/v2/libparse.so.1 symver/app_old symver/app_new glibc/th glibc/fm_old glibc/th_el7 \
         sysroot-el7/lib64/libc-2.17.so glibc/libc-native.so; do
  $RE --dyn-syms -W "$W/$f" | awk '$1 ~ /^[0-9]+:$/ { $2=$3=$4=$5=$6=$7=""; print }' | tr -s ' ' > $W/verdump.re
  $V "$W/$f" | tr -s ' ' | sed 's/^ //' > $W/verdump.me
  if diff $W/verdump.re $W/verdump.me >/dev/null; then
    echo "same: $f ($(wc -l < $W/verdump.me | tr -d ' ') symbols)"
  else
    echo "DIFF: $f"; diff $W/verdump.re $W/verdump.me | head; exit 1
  fi
done
