#!/bin/sh
# 静态初始化顺序：链接顺序决定 .init_array 里各翻译单元初始化函数的顺序
. "$(dirname "$0")/../env.sh"; stage init
set -x
for f in base derived main; do $GXX -O1 -c $f.cpp; done
$NM base.o
$RE -SW base.o | grep init_array
$RE -rW base.o | grep -A2 'rela.init_array'
ok $GXX -static main.o base.o derived.o -o bd && run_x86 ./bd
ok $GXX -static main.o derived.o base.o -o db && run_x86 ./db
$OD -s -j .init_array bd; $NM bd | grep -E '_GLOBAL__sub_I|frame_dummy'
link_lld() { out=$1; shift; $GXX -static -fuse-ld=lld "$@" -o "$out"; }
ok link_lld bd_lld main.o base.o derived.o && run_x86 ./bd_lld
ok link_lld db_lld main.o derived.o base.o && run_x86 ./db_lld
# --sort-section 作用于链接脚本里的通配符模式，不改变同名 .init_array 输入节之间的顺序
ok $GXX -static -Wl,--sort-section=name main.o derived.o base.o -o db_sort && run_x86 ./db_sort
# init_priority：base 的初始化函数进入 .init_array.00101
$GXX -O1 -DPRIO=101 -c base.cpp -o base_p.o
$RE -SW base_p.o | grep init_array
ok $GXX -static main.o derived.o base_p.o -o db_prio && run_x86 ./db_prio
ok link_lld db_prio_lld main.o derived.o base_p.o && run_x86 ./db_prio_lld
ld.bfd --verbose | grep -A4 '^  .init_array'
$GXX -O1 -DPRIO=50 -c base.cpp -o base_50.o 2>&1 | head -2
# 修法：函数内的 static（第一次使用时构造），链接顺序不再影响结果
for f in base_fix derived_fix main_fix; do $GXX -O1 -c $f.cpp; done
ok $GXX -static main_fix.o derived_fix.o base_fix.o -o fix_db && run_x86 ./fix_db
ok $GXX -static main_fix.o base_fix.o derived_fix.o -o fix_bd && run_x86 ./fix_bd
# .ctors.N：GNU ld 把它并进 .init_array，lld 留成单独的 .ctors
clang -c ctors.s -o ctors.o
ld.bfd ctors.o -o ctors_bfd && $RE -SW ctors_bfd | grep -E 'ctors|init_array'
ld.lld ctors.o -o ctors_lld && $RE -SW ctors_lld | grep -E 'ctors|init_array'
exit $FAIL
