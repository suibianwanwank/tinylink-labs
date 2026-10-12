#!/bin/sh
# 开头的谜题：两个翻译单元各生成一份 twice、add<int>、next_id
. "$(dirname "$0")/../env.sh"; stage puzzle
set -x
$GXX -O0 -c a.cpp && $GXX -O0 -c b.cpp
$NM a.o; $NM b.o
$RE -gW a.o
$RE -sW a.o | grep -E 'twice|add|next_id'
$GXX -static a.o b.o -o ab
$NM ab | grep -E 'twice|addI|next_id'
run_x86 ./ab
# 同一台 Linux 的 GCC 默认使用 STB_GNU_UNIQUE；Clang 的选择在下方对照
run_native sh -c 'g++ -O0 -c a.cpp -o a_deb.o && nm a_deb.o && readelf -sW a_deb.o | grep _ZZ'
# clang 的结果
$CXX_CLANG -O0 -c a.cpp -o a_clang.o && $NM a_clang.o
# 同一函数两份留几份：看最终产物里 .text 的 twice 只有一个地址
$OD -d ab --no-show-raw-insn | grep -c '<_Z5twicei>:'
exit $FAIL
