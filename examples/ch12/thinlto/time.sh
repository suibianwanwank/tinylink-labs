#!/bin/sh
# Linux 原生链接计时、ThinLTO 缓存和最大常驻内存；随后直接执行三个产物。
set -e
. "$(dirname "$0")/../env.sh"
cd "$W/src/lua-5.4.7"
t() { label=$1; shift; best=999; for i in 1 2 3 4 5; do s=$(perl -MTime::HiRes=time -e 'printf "%.4f", time'); "$@" >/dev/null 2>&1 || { echo FAIL $label; "$@"; return; }; e=$(perl -MTime::HiRes=time -e 'printf "%.4f", time'); d=$(echo "$e - $s" | bc); echo "$label run$i $d"; done; }
t none link_musl lua-none obj-none/*.o
t full link_musl lua-full obj-full/*.o
t thin-1 link_musl lua-thin obj-thin/*.o --thinlto-jobs=1
t thin-all link_musl lua-thin obj-thin/*.o
now() { perl -MTime::HiRes=time -e 'printf "%.4f", time'; }
tm() { s=$(now); "$@" >/dev/null 2>&1; e=$(now); echo "$(echo "$e - $s" | bc)"; }
mkdir -p cache && find cache -mindepth 1 -delete
echo "cold: $(tm link_musl lua-thin obj-thin/*.o --thinlto-cache-dir=cache)"
echo "warm: $(tm link_musl lua-thin obj-thin/*.o --thinlto-cache-dir=cache)"
echo "warm: $(tm link_musl lua-thin obj-thin/*.o --thinlto-cache-dir=cache)"
ls cache | wc -l
cp src/lcorolib.c $W/lcorolib.c.bak
# 改一个文件：给 coroutine.running 换一个实现细节
SYSINC="-nostdinc -isystem /usr/include/x86_64-linux-musl"
sed 's/lua_pushboolean(L, ismain);/lua_pushboolean(L, ismain != 0);/' $W/lcorolib.c.bak > src/lcorolib.c
(cd src && $CC $SYSINC -O2 -DLUA_USE_POSIX -flto=thin -c lcorolib.c -o ../obj-thin/lcorolib.o)
echo "one file changed: $(tm link_musl lua-thin obj-thin/*.o --thinlto-cache-dir=cache)"
ls cache | wc -l
cp $W/lcorolib.c.bak src/lcorolib.c
(cd src && $CC $SYSINC -O2 -DLUA_USE_POSIX -flto=thin -c lcorolib.c -o ../obj-thin/lcorolib.o)
mem() { out=$1; shift; /usr/bin/time -f 'max_rss_kib=%M' ld.lld -static -o "$out" "$SR/crt1.o" "$SR/crti.o" "$@" -L"$SR" -lc "$SR/crtn.o" 2>&1; }
echo "none $(mem lua-none obj-none/*.o)"
echo "full $(mem lua-full obj-full/*.o)"
for j in 1 2; do echo "thin jobs=$j $(mem lua-thin obj-thin/*.o --thinlto-jobs=$j)"; done
echo "thin default $(mem lua-thin obj-thin/*.o)"
llvm-size lua-none lua-full lua-thin
mkdir -p "$W/lua" && cp lua-none lua-full lua-thin "$W/lua/"
(cd "$W/lua"; sh -c 'for p in none full thin; do ./lua-$p -e "local t=0 for i=1,3e7 do t=t+i%7 end print(_VERSION, t)"; done')
