#!/usr/bin/env bash
# 验证 run-linux.sh：
#   hello.c  正常退出，stdout 和退出码 42 都要对；
#   crash.c  段错误，要在超时之内返回 139（128 + SIGSEGV 的编号 11）；
#   spin.c   死循环，要在超时后被杀掉，返回 124；
#   two.out  tinylink 把两个没有重定位的目标文件链接成的程序（实验 3），退出码 42。
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
cc=${MUSL_CC:-x86_64-linux-musl-gcc}
run=$here/run-linux.sh

for p in hello crash spin; do
    "$cc" -static -O1 -o "$here/smoke/$p.out" "$here/smoke/$p.c"
done
file "$here/smoke/hello.out"

status=0
output=$("$run" "$here/smoke/hello.out" tinylink) || status=$?
echo "$output"
echo "hello exit status: $status"

crash=0
LINKER_LAB_TIMEOUT=20 "$run" "$here/smoke/crash.out" || crash=$?
echo "crash exit status: $crash"

spin=0
LINKER_LAB_TIMEOUT=3 "$run" "$here/smoke/spin.out" || spin=$?
echo "spin exit status: $spin"

root=$(cd "$here/.." && pwd)
(cd "$root" && cargo build -q)
objs=$root/tests/resolve/golden
"$root/target/debug/tinylink" link -o "$here/smoke/two.out" "$objs/start.o" "$objs/blob.o"
two=0
"$run" "$here/smoke/two.out" || two=$?
echo "two exit status: $two"

expected="hello from x86-64, argc=2, argv[1]=tinylink"
if [[ $output == "$expected" && $status -eq 42 && $crash -eq 139 && $spin -eq 124 && $two -eq 42 ]]; then
    echo "smoke test passed"
else
    echo "smoke test FAILED" >&2
    exit 1
fi
