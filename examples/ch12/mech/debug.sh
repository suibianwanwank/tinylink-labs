#!/bin/sh
# 第 12 章"调试信息也在链接时生成"：-g -flto 的 bitcode 里只有元数据，DWARF 由 LTO 后端生成。
set -e
. "$(dirname "$0")/../env.sh"
mkdir -p "$W/lto-g" && cp "$(dirname "$0")"/../puzzle/main.c "$(dirname "$0")"/../puzzle/util.c "$W/lto-g/" && cd "$W/lto-g"
set -x
$CC -O2 -g -flto -c main.c -o main.o
$CC -O2 -g -flto -c util.c -o util.o
file main.o
llvm-dis main.o -o - | grep -E '^!.* = distinct !DICompileUnit|^!.* = distinct !DISubprogram'
link_musl prog main.o util.o --save-temps
llvm-readelf -SW prog.lto.o | grep -E '\.debug_'
llvm-dwarfdump --debug-info prog.lto.o | grep -E 'DW_TAG_(compile_unit|subprogram|inlined_subroutine)|DW_AT_abstract_origin|DW_AT_name\t\("(main|util)\.c"\)|DW_AT_name\t\("(main|scale)"\)'
