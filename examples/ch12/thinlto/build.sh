#!/bin/sh
# 用 Lua 5.4.7 的 33 个 C 文件比较 full LTO 与 ThinLTO 的链接时间。
# 源码：https://www.lua.org/ftp/lua-5.4.7.tar.gz
# sha256 9fbf5e28ef86c69858f6d3d34eccc32e911c1a28b4120ff3e84aaa70cfbf1e30
set -e
. "$(dirname "$0")/../env.sh"
mkdir -p "$W/src" && cd "$W/src"
[ -f lua-5.4.7.tar.gz ] || curl -sSfLO https://www.lua.org/ftp/lua-5.4.7.tar.gz
[ -d lua-5.4.7 ] || tar xzf lua-5.4.7.tar.gz
cd lua-5.4.7/src
SYSINC="-nostdinc -isystem /usr/include/x86_64-linux-musl"
SRCS=$(ls *.c | grep -v '^luac.c$')
for mode in none full thin; do
  mkdir -p "../obj-$mode" && find "../obj-$mode" -mindepth 1 -delete
  case $mode in none) F="";; full) F="-flto";; thin) F="-flto=thin";; esac
  for f in $SRCS; do $CC $SYSINC -O2 -DLUA_USE_POSIX $F -c "$f" -o "../obj-$mode/${f%.c}.o"; done
done
echo "files: $(echo $SRCS | wc -w)"
