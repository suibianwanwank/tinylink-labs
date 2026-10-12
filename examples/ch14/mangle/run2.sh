#!/bin/sh
# extern "C" 与返回类型
. "$(dirname "$0")/../env.sh"; stage mangle2
set -x
musl-gcc -O1 -c main_c.c
$GXX -O1 -c area.cpp -o area.o
$NM area.o
$GXX -static main_c.o area.o -o area_bad
$GXX -O1 -DWITH_EXTERN_C -c area.cpp -o area_c.o
$NM area_c.o
ok $GXX -static main_c.o area_c.o -o area_ok && run_x86 ./area_ok
$GXX -O1 -c ratio.cpp && $GXX -O1 -c use_ratio.cpp
$NM ratio.o use_ratio.o | grep ratio
ok $GXX -static ratio.o use_ratio.o -o ratio_bad && run_x86 ./ratio_bad
$OD -d --no-show-raw-insn ratio.o | sed -n '/<_Z5ratioi>:/,$p'
$GXX -O1 -DWRONG_PARAM -c use_ratio.cpp -o use_ratio2.o
$GXX -static ratio.o use_ratio2.o -o ratio_bad2
ld.lld -static -o ratio_bad2 ratio.o use_ratio2.o 2>&1 | head -5
exit $FAIL
