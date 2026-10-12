#!/bin/sh
# 第 16 章：在原生 x86-64 Linux 上编译、链接并执行。
# 每次创建独立目录；LINKER_LAB_DIR 可指定一个尚不存在或为空的目录。
set -eu
ulimit -c 0
here=$(cd "$(dirname "$0")" && pwd)
mkdir -p "$HOME/.cache/linker-lab"
lab=${LINKER_LAB_DIR:-$(mktemp -d "$HOME/.cache/linker-lab/ch16.XXXXXX")}
mkdir -p "$lab"
[ -z "$(ls -A "$lab")" ] || { echo "experiment directory must be empty: $lab" >&2; exit 2; }
lab=$(cd "$lab" && pwd)
echo "experiment directory: $lab"
[ "$(uname -s)" = Linux ] && [ "$(uname -m)" = x86_64 ] || { echo "requires native x86-64 Linux" >&2; exit 2; }
CC=${CC:-musl-gcc}; AR=${AR:-ar}; NM=${NM:-nm}; RE=${RE:-readelf}
run() { timeout --kill-after=2s 30s "$@"; }

echo "== 实验一：debug_dump 为什么没被 --gc-sections 删掉"
mkdir "$lab/live" && cp "$here"/live/*.c "$lab/live/" && cd "$lab/live"
clang -O2 -ffunction-sections -fdata-sections -c main.c handlers.c
ld.lld -e main --gc-sections --why-live='debug_*' main.o handlers.o -o live
echo "-- 练习一"
ld.lld -e main --gc-sections --why-live=on_read main.o handlers.o -o live

echo "== 实验二：弱的默认值被静态库里的强定义顶替"
mkdir "$lab/weak" && cp "$here"/weak/*.c "$lab/weak/" && cd "$lab/weak"
"$CC" -O2 -c app.c net.c
"$AR" rcs libnet.a net.o
"$NM" -A app.o net.o | grep log_level
"$CC" -static -fuse-ld=lld -Wl,--no-dynamic-linker app.o -L. -lnet -o app \
  -Wl,-y,log_level -Wl,--why-extract=why.txt
grep -v libc.a why.txt
run "$lab/weak/app"
echo "-- 练习二：去掉 net_init() 的调用"
sed '/    net_init();/d' app.c > app2.c
"$CC" -O2 -c app2.c
"$CC" -static -fuse-ld=lld -Wl,--no-dynamic-linker app2.o -L. -lnet -o app2 \
  -Wl,-y,log_level -Wl,--why-extract=why2.txt
grep -v libc.a why2.txt
run "$lab/weak/app2"

echo "== 实验三：第 0 章的类型陷阱，nm -S 与 GCC 的 -flto"
mkdir "$lab/typecheck" && cp "$here"/typecheck/*.c "$lab/typecheck/" && cd "$lab/typecheck"
"$CC" -O2 -c main.c other.c
"$NM" -S main.o other.o | grep -w x
"$CC" -O2 -flto -c main.c -o main.lto.o
"$CC" -O2 -flto -c other.c -o other.lto.o
"$CC" -O2 -flto -static main.lto.o other.lto.o -o prog
run "$lab/typecheck/prog"

echo "== 练习三：-z cet-report=error，再手写 .note.gnu.property 修好"
mkdir "$lab/cet" && cp "$here"/cet/* "$lab/cet/" && cd "$lab/cet"
clang -O2 -fcf-protection=full -c p.c -o p_cet.o
clang -c fast_add.s -o fast_add.o
clang -c fast_add_cet.s -o fast_add_cet.o
if ld.lld -shared -z cet-report=error p_cet.o fast_add.o -o mix.so; then
 echo "expected CET property rejection" >&2; exit 1
fi
ld.lld -shared -z cet-report=error p_cet.o fast_add_cet.o -o fixed.so
"$RE" -nW fixed.so | grep Properties
ld.lld -shared -z force-ibt p_cet.o fast_add.o -o force.so
"$RE" -nW force.so | grep Properties
