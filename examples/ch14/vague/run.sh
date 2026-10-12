#!/bin/sh
# vague linkage 的两种实现：COMDAT 组与弱符号，四种组合各链接一次
. "$(dirname "$0")/../env.sh"; stage vague
AS="clang -c"
for v in "weak+comdat:-DWEAK -DCOMDAT" "comdat:-DCOMDAT" "weak:-DWEAK" "neither:"; do
  name=${v%%:*}; flags=${v#*:}
  echo "======== $name"
  $AS -DFILE=1 $flags twice.S -o t1_$name.o
  $AS -DFILE=2 $flags twice.S -o t2_$name.o
  $NM t1_$name.o | grep twice
  for L in ld.bfd ld.lld; do
    if $L -e user1 t1_$name.o t2_$name.o -o out_$name 2>err; then
      printf '%s: ok, twice 的副本数 = %s\n' $L "$($OD -d out_$name | grep -c 'lea ')"
    else
      printf '%s: ' $L; head -3 err
    fi
  done
done
echo "======== weak + --gc-sections（_start 调用 user1 和 user2）"
$AS gcmain.s -o gcmain.o
for L in ld.bfd ld.lld; do
  $L -e _start gcmain.o t1_weak.o t2_weak.o -o gc0 && $L --gc-sections -e _start gcmain.o t1_weak.o t2_weak.o -o gc1
  printf '%s: 不加 %s 份，加 --gc-sections %s 份\n' $L "$($OD -d gc0 | grep -c 'lea ')" "$($OD -d gc1 | grep -c 'lea ')"
done
exit $FAIL
