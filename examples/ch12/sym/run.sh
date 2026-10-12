#!/bin/sh
# 第 12 章"哪些符号必须保留"：--export-dynamic、版本脚本、内联汇编、__attribute__((used))。
set -e
. "$(dirname "$0")/../env.sh"
mkdir -p "$W/sym" && cp "$(dirname "$0")"/*.c "$(dirname "$0")"/*.map "$W/sym/" && cd "$W/sym"
set -x
$CC -O2 -flto -c app.c -o app.o
link_musl app app.o
llvm-nm app | grep -E ' (main|helper|plugin_hook)$' || true
link_musl_dyn app_dyn app.o --save-temps
cat app_dyn.resolution.txt
llvm-nm app_dyn | grep -E " (main|helper|plugin_hook)$" || true
link_musl_dyn app_ed app.o --export-dynamic --save-temps
cat app_ed.resolution.txt
llvm-nm app_ed | grep -E ' (main|helper|plugin_hook)$' || true
link_musl app_st app.o --save-temps; cat app_st.resolution.txt
llvm-nm -D --defined-only app_ed
$CC -O2 -fPIC -flto -c lib.c -o lib.o
ld.lld -shared lib.o -o lib_all.so
llvm-nm -D --defined-only lib_all.so
ld.lld -shared --version-script=lib.map lib.o -o lib_map.so
llvm-nm -D --defined-only lib_map.so
llvm-nm lib_map.so | grep -E 'api_get|impl_detail' || true
# 内联汇编里的引用
$CC -O2 -c asm_main.c -o asm_main.o
$CC -O2 -c asm_util.c -o asm_util.o
link_musl asm_plain asm_main.o asm_util.o && echo plain-ok
$CC -O2 -flto -c asm_main.c -o asm_main.lto.o
$CC -O2 -flto -c asm_util.c -o asm_util.lto.o
link_musl asm_lto asm_main.lto.o asm_util.lto.o --save-temps || true
cat asm_lto.resolution.txt
$CC -O2 -flto -c asm_util_used.c -o asm_util_used.lto.o
link_musl asm_used asm_main.lto.o asm_util_used.lto.o && llvm-nm asm_used | grep -E ' (main|helper2)$'
# 文件级汇编里定义的符号
$CC -O2 -flto -c toplevel.c -o toplevel.lto.o
$CC -O2 -flto -c top_main.c -o top_main.lto.o
llvm-nm toplevel.lto.o
link_musl top top_main.lto.o toplevel.lto.o && llvm-nm top | grep -E ' (main|asm_seven)$'
run_x86 sh -c 'cd sym; ./asm_plain; echo asm_plain=$?; ./asm_used; echo asm_used=$?; ./top; echo top=$?; ./app; echo app=$?; ./app_ed; echo app_ed=$?'
