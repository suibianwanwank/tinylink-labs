#!/bin/sh
# key function：vtable 只放在定义了 key function 的那个 .o 里；忘了定义就报 undefined reference to vtable
. "$(dirname "$0")/../env.sh"; stage key
set -x
$GXX -O0 -c shape.cpp && $GXX -O0 -c use_shape.cpp
$NM -C shape.o | grep -v ' U '
$NM -C use_shape.o
$RE -gW shape.o | grep 'group section'
ok $GXX -static use_shape.o shape.o -o ok && run_x86 ./ok
$GXX -static use_shape.o -o bad
ld.lld -static -o bad use_shape.o 2>&1 | grep -A3 "vtable for" | head -4
exit $FAIL
