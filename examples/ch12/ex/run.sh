#!/bin/sh
# 第 12 章练习。先运行 ../puzzle/run.sh。
set -e
. "$(dirname "$0")/../env.sh"
HERE=$(cd "$(dirname "$0")" && pwd)
mkdir -p "$W/ex" && cp "$HERE"/*.c "$HERE"/dyn.list "$W/ex/" && cd "$W/ex"
set -x
# 练习一：观察。--lto-O0；GCC 的 fat 对象交给 lld
link_musl lto_o0 ../puzzle/main.lto.o ../puzzle/util.lto.o --lto-O0
llvm-nm -S lto_o0 | grep -E ' (main|scale|twice|checksum)$'
llvm-objdump -d --no-show-raw-insn --disassemble-symbols=main lto_o0 | grep call
gcc -O2 -flto -ffat-lto-objects -c ../puzzle/main.c -o main.fat.o
gcc -O2 -flto -ffat-lto-objects -c ../puzzle/util.c -o util.fat.o
link_musl fat main.fat.o util.fat.o
llvm-nm -S fat | grep -E ' (main|scale|twice|checksum)$'
llvm-objdump -d --no-show-raw-insn --disassemble-symbols=main fat | grep call
# 练习二：手算。地址表与 safe/all
$CC -O2 -ffunction-sections -c taken.c -o taken.o
llvm-objdump -s -j .llvm_addrsig taken.o
llvm-readelf -sW taken.o | grep -E 'FUNC|OBJECT'
link_musl taken_safe taken.o --icf=safe --print-icf-sections 2>&1 | grep -A3 'taken.o:(.text.'
link_musl taken_all taken.o --icf=all --print-icf-sections 2>&1 | grep -A3 'taken.o:(.text.'
# 练习二：互相调用的函数
$CC -O0 -ffunction-sections -c mutual.c -o mutual0.o
llvm-readelf -r mutual0.o | grep -E 'rela.text.[abcd]|PLT32' | head -8
link_musl mutual0 mutual0.o --icf=all --print-icf-sections 2>&1 | grep -A2 'mutual0.o:(.text.'
llvm-nm mutual0 | grep -E ' [abcd]$' | sort
link_musl mutual0_safe mutual0.o --icf=safe --print-icf-sections 2>&1 | grep -A2 'mutual0.o:(.text.' || true
llvm-objdump -s -j .llvm_addrsig mutual0.o | tail -1
# 练习三：改坏。dlopen 的插件找不到主程序里的符号
SYS="-nostdinc -isystem /usr/include/x86_64-linux-musl"
$CC -O2 -fPIC -c plugin.c -o plugin.o && ld.lld -shared plugin.o -o plugin.so
$CC $SYS -fPIE -O2 -c host.c -o host.o
$CC $SYS -fPIE -O2 -flto -c host.c -o host.lto.o
sed 's/^int plugin_hook/__attribute__((used)) int plugin_hook/' host.c > host_used.c
$CC $SYS -fPIE -O2 -flto -c host_used.c -o host_used.lto.o
link_musl_dyn h_plain host.o
link_musl_dyn h_plain_E host.o --export-dynamic
link_musl_dyn h_lto host.lto.o
link_musl_dyn h_lto_used host_used.lto.o
link_musl_dyn h_lto_E host.lto.o --export-dynamic
link_musl_dyn h_lto_dl host.lto.o --dynamic-list=dyn.list
for f in h_plain h_lto h_lto_used h_lto_E h_lto_dl; do
  echo "$f symtab=$(llvm-nm $f | grep -c ' plugin_hook$') dynsym=$(llvm-nm -D --defined-only $f | grep -c plugin_hook)"
done
run_x86 sh -c 'cd ex; for f in h_plain h_plain_E h_lto h_lto_used h_lto_E h_lto_dl taken_safe taken_all mutual0; do echo "== $f"; ./$f a; echo "exit=$?"; done'
