#!/bin/sh
# guard 变量：__cxa_guard_acquire/release 与 _ZGV 前缀
. "$(dirname "$0")/../env.sh"; stage guard
set -x
$GXX -O1 -c guard.cpp && $GXX -O1 -c main.cpp
$NM guard.o
$NM guard.o | awk '{print $NF}' | grep _ZGV | c++filt
$RE -gW guard.o | grep -A3 _ZGV
$RE -sW guard.o | grep -E '_ZGV|_ZZ'
$OD -d -r --no-show-raw-insn guard.o | sed -n '/<_Z6configv>:/,/^$/p'
ok $GXX -static main.o guard.o -o g -Wl,--trace-symbol=__cxa_guard_acquire && run_x86 ./g
$GXX -O1 -fno-threadsafe-statics -c guard.cpp -o guard_nts.o
$NM guard_nts.o | grep -E 'cxa|_ZGV'
$OD -d -r --no-show-raw-insn guard_nts.o | sed -n '/<_Z6configv>:/,/^$/p'
exit $FAIL
