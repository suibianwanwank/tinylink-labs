#!/bin/sh
# 第 14 章练习一到三。先跑 vague/key.sh（练习一用它的 shape.o）；Rust 练习直接使用提供的 nostd/tiny.rs
. "$(dirname "$0")/../env.sh"
SRC=$(cd "$(dirname "$0")/.." && pwd)
stage ex
echo "######## 练习一 (1)：Shape 的虚表（vague/key.sh 的 shape.o）"
K=$W/key
$RE -SW $K/shape.o | grep -E '_ZTV5Shape'
$RE -rW $K/shape.o | sed -n '/rela.rodata._ZTV5Shape/,/^$/p'
$NM $K/shape.o | grep -E 'D[0-9]Ev'
$RE -gW $K/shape.o | grep -A5 'D5Ev'
echo "######## 练习一 (2)：标准库 rlib 的成员"
HOST=$(rustc -vV | sed -n 's/^host: //p')
S=$(rustc --print sysroot)/lib/rustlib/$HOST/lib
for f in libstd libcore libcompiler_builtins; do
  r=$(ls $S/$f-*.rlib); printf '%s: %s 个成员，其中 .rcgu.o %s 个\n' $f "$(ar t $r | wc -l | tr -d ' ')" "$(ar t $r | grep -c rcgu)"
done
ar t $S/libcompiler_builtins-*.rlib | grep -v rcgu | sed -n '3,6p'
nm -A $S/libcompiler_builtins-*.rlib 2>/dev/null | grep -E ' [TW] (__udivti3|__popcountdi2|memcpy)$' | sed 's|.*rlib:||'
echo "######## 练习二：手算的修饰名，用编译器和 c++filt 核对"
$GXX -c handcalc.cpp && $NM handcalc.o
echo _ZN5shape6circle6resizeERKS0_d | c++filt
echo _RNvNtCs1234_7mycrate3foo3bar | llvm-cxxfilt; echo _RNvNtCs1234_7mycrate3foo3bar | c++filt
echo "######## 练习三 (1)：多了一个没人调用的虚函数"
$GXX -O0 -c shape.cpp && $GXX -O0 -c use_shape.cpp
$NM -C shape.o | grep -E 'vtable|area|draw'
$GXX -static use_shape.o shape.o -o s1 2>&1 | grep -oE "undefined reference to \`[^']*'" | sort | uniq -c
$GXX -O0 -DFIX -c shape.cpp -o shape_fix.o && $NM -C shape_fix.o | grep -E 'vtable|draw'
ok $GXX -static use_shape.o shape_fix.o -o s2 && run_x86 ./s2
echo "######## 练习三 (2)：_start 去掉 no_mangle"
# Remove only the attribute on _start; retain rust_entry's spelling.
awk '/^#\[unsafe\(no_mangle\)\]$/ { attribute=$0; next }
     { if (attribute != "" && $0 !~ /^pub extern "C" fn _start/) print attribute;
       attribute=""; print }' "$SRC/rust/nostd/tiny.rs" > tiny_bad.rs
ok rustc --edition 2024 -C panic=abort -C opt-level=2 \
  -C linker-flavor=ld -C link-arg=-static -C relocation-model=static \
  tiny_bad.rs -o tiny_bad
$NM tiny_bad; $RE -h tiny_bad | grep Entry
run_x86 sh -c './tiny_bad; echo exit=$?'
exit $FAIL
