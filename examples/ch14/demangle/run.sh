#!/bin/sh
# 练习四：mini-demangle 与 GNU c++filt 对拍
. "$(dirname "$0")/../env.sh"
D=$(cd "$(dirname "$0")" && pwd)
(cd "$D" && cargo build -q --release && cargo test -q 2>&1 | grep 'test result')
MD=$D/target/release/mini-demangle
stage demangle
$GXX -c cases.cpp -o cases.o
$GXX -c "$D/../mangle/names.cpp" -o names.o
LIBSTDCXX=$($GXX -print-file-name=libstdc++.a)
# 再加上正文里几个带 vtable、typeinfo、guard 和局部静态变量的目标文件
EXTRA="$W/puzzle/a.o $W/guard/guard.o $W/key/shape.o $W/odr/odr_a.o"
{ $NM cases.o names.o; $NM $EXTRA 2>/dev/null; $NM "$LIBSTDCXX" 2>/dev/null; } | awk 'NF>=2 {print $NF}' | grep '^_Z' | sort -u > all.txt
$MD < all.txt > mine.txt 2> unsupported.txt
c++filt < all.txt > ref.txt
paste all.txt mine.txt ref.txt | awk -F'\t' '$1 != $2 {n++; if ($2 == $3) ok++; else {bad++; print "DIFF: " $1 "\n  mine: " $2 "\n  ref:  " $3}} END {printf "符号总数 %d，支持 %d，与 c++filt 一致 %d，不一致 %d\n", NR, n, ok, bad+0; exit bad != 0}' || exit 1
echo "特殊名字："; paste all.txt mine.txt | awk -F'\t' '$1 != $2' | grep -E '^_Z(TV|TI|TS|GV|Z)' | grep -E '1a1V|cfg|count_k|Shape|next_id' | head -12
echo "示例："; paste all.txt mine.txt | awk -F'\t' '$1 != $2' | grep -E 'cases|^_ZN1a|^_Z2h|^_ZSt|^_ZN3geo' | head -14

# 实验 14 的验收形式：链接器报 undefined reference 时显示 demangle 后的名字
echo "== 链接器不 demangle 时的报错，交给 mini-demangle"
cd "$W/key"
sym=$($GXX -static -Wl,--no-demangle use_shape.o -o bad 2>&1 | grep -o "undefined reference to \`[^']*'" | head -1 | sed "s/.*\`//; s/'//")
echo "undefined reference to \`$sym'  ->  undefined reference to \`$($MD "$sym")'"
exit $FAIL
