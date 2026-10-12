#!/usr/bin/env bash
# 原生 x86-64 Linux TLS 练习。AArch64 只作 ABI 指令/文件布局对照，不运行。
set -euo pipefail
[[ $(uname -s) == Linux && $(uname -m) == x86_64 ]]
source_dir=$(cd "$(dirname "$0")" && pwd)
work=$(mktemp -d "${TMPDIR:-/tmp}/linker-tls-exercises.XXXXXX")
echo "experiment directory: $work"
cp "$source_dir/"{rt.c,q.c,start.s,big.c,big_gd.c,host.c} "$work/"
cd "$work"
gcc --version | head -n 1
ld --version | head -n 1
ldd --version | head -n 1
gcc -O1 rt.c -pthread -o rt-x86
./rt-x86
readelf -lW rt-x86 | grep TLS
rustc -O --edition 2024 "$source_dir/tpoff/main.rs" -o tpoff
clang -O1 -c q.c -o q.o
clang -c start.s -o start.o
ld.lld -pie start.o q.o -o q-x86_64
# Only a format/ABI comparison; no foreign binary execution or emulation.
clang --target=aarch64-unknown-linux-gnu -O1 -c q.c -o q-a64.o
clang --target=aarch64-unknown-linux-gnu -c start.s -o start-a64.o
ld.lld -pie start-a64.o q-a64.o -o q-aarch64
for p in q-x86_64 q-aarch64 rt-x86; do echo "## $p"; ./tpoff "$p"; done
llvm-objdump -d q-x86_64
llvm-objdump -d q-aarch64
gcc -O1 host.c -o host -ldl
for n in 1024 1664 1665 1712 1713 2048 2176 2177 2224 2225 4096 8192; do
    gcc -O1 -fPIC -shared -DN="$n" big.c -o "libbig$n.so"
    status=0
    output=$(./host "./libbig$n.so") || status=$?
    echo "N=$n status=$status: $output"
done
readelf -dW libbig2048.so | grep FLAGS
readelf -rW libbig2048.so | grep TPOFF
# The boundary is environment dependent: search it in a fresh process for each candidate.
low=0; high=4096
while (( high-low > 1 )); do
 n=$(((low+high)/2))
 gcc -O1 -fPIC -shared -DN="$n" big.c -o libprobe.so
 if ./host ./libprobe.so >/dev/null; then low=$n; else high=$n; fi
done
echo "default boundary: last success $low, first failure $high"
gcc -O1 -fPIC -shared -DN=4096 big_gd.c -o libgd4096.so
./host ./libgd4096.so
for n in 2048 2176 2177 2224 2225; do
 status=0
 output=$(GLIBC_TUNABLES=glibc.rtld.optional_static_tls=1024 ./host "./libbig$n.so") || status=$?
 echo "tunable N=$n status=$status: $output"
done
gcc -O1 host.c -Wl,--no-as-needed ./libbig8192.so -o host_dep -ldl
readelf -dW host_dep | grep NEEDED
./host_dep ./libbig8192.so
echo 'native TLS exercises complete'
