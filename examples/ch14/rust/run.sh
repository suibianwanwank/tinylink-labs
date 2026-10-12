#!/bin/sh
# Rust 的链接：rlib、修饰、属性、rustc 调用链接器、依赖顺序、panic 策略、no_std
. "$(dirname "$0")/../env.sh"; stage rust
T=x86_64-unknown-linux-musl
RC="rustc --edition 2024 --target $T -C symbol-mangling-version=v0"
LK="-C linker=musl-gcc"
SYS=$(rustc --print sysroot)
# 把 --print link-args 的输出拆成一行一个参数，长路径换成短名
args() { tr ' ' '\n' | sed -En '/^"(musl-gcc|rust-lld)"$/,$p' | sed -E "s|$SYS|\$SYSROOT|g; s|$PWD/||g; s|/[^\"]*/rustc[A-Za-z0-9]+/|<tmp>/|g"; }
rustc -V
echo "######## rlib：一个 ar 归档"
cd rlib
$RC --crate-type rlib -O geom.rs
ar tv libgeom.rlib
# lib.rmeta 里带着源文件的绝对路径：换个目录编译，大小就变；--remap-path-prefix 之后逐字节相同
for d in a longer-directory-name; do
  mkdir -p rp/$d && cp geom.rs rp/$d/
  (cd rp/$d && $RC --crate-type rlib -O geom.rs && ar tv libgeom.rlib | grep 'lib.rmeta$' \
     && $RC --crate-type rlib -O --remap-path-prefix="$PWD=/src" geom.rs -o libgeom_remap.rlib)
done
strings rp/a/libgeom.rlib | grep -m1 "$PWD/rp/a/geom.rs" | sed "s|$W|\$W|"
cmp rp/a/libgeom_remap.rlib rp/longer-directory-name/libgeom_remap.rlib && echo "remap 后两份 rlib 相同"
mkdir -p x && (cd x && ar x ../libgeom.rlib && file * && $NM *.rcgu.o && $RE -SW lib.rmeta | grep -E '\.rmeta')
echo "######## 修饰：显式 v0 与本机 stable 的默认 legacy"
$NM libgeom.rlib 2>/dev/null | grep ' T '
rustc --edition 2024 --target $T --crate-type rlib -O geom.rs -o libgeom_legacy.rlib
$NM libgeom_legacy.rlib 2>/dev/null | grep ' T '
for s in $($NM libgeom_legacy.rlib 2>/dev/null | awk '$2=="T" {print $3}') $($NM libgeom.rlib 2>/dev/null | awk '$2=="T" {print $3}') \
         _RINvNtCs806nzKq9KaB_4core9panicking13assert_failedllECsl1RqEF7rHWY_3std; do
  echo "== $s"; echo $s | llvm-cxxfilt; echo $s | c++filt
done
echo "######## 下游 crate 使用 geom：泛型实例归谁"
$RC -O --extern geom=libgeom.rlib app.rs --emit obj -o app.o -C codegen-units=1
$NM app.o | grep -v ' U ' | head -12
$RE -gW app.o | grep 'group section'
cd ../generics
for O in 0 2; do
  echo "== opt-level=$O"
  $RC --crate-type rlib -C opt-level=$O geom.rs -o libgeom$O.rlib
  $RC -C opt-level=$O --extern geom=libgeom$O.rlib app.rs --emit obj -o app$O.o -C codegen-units=1
  $NM libgeom$O.rlib 2>/dev/null | grep biggest; $NM app$O.o | grep biggest
done
echo "######## no_mangle、extern \"C\"、used、link_section"
cd ../attrs
$RC -O -C codegen-units=1 attrs.rs --emit obj -o attrs.o 2>&1 | grep warning
$NM attrs.o | tee nm.txt; awk '{print $3}' nm.txt | llvm-cxxfilt
$RE -SW attrs.o | grep -E 'rs_|note.mytag|KEEP'
$OD -s -j .note.mytag attrs.o | tail -1
echo "== C 的 used 与 used+retain"
clang -O2 -c used.c -o used_clang.o && $RE -SW used_clang.o | grep rodata
musl-gcc -O2 -c used.c -o used_gcc.o && $RE -SW used_gcc.o | grep rodata
echo "######## rustc 怎样调用链接器"
cd ../linkargs
$RC -O hello.rs -o hello_cc || exit 1
file hello_cc; run_x86 ./hello_cc
$RC -O $LK -C link-arg=-Wl,-Map=hello.map --print link-args hello.rs -o hello | args
file hello; run_x86 ./hello
$RC -O $LK -C save-temps hello.rs -o hello2
find . -maxdepth 2 -type f -name "*.rcgu.o" -print
echo "######## 依赖顺序：app -> util -> geom -> libcfoo.a、libm"
cd ../deps
musl-gcc -O1 -c cfoo.c && ar rcs libcfoo.a cfoo.o
$RC --crate-type rlib -O -L . geom.rs && ar t libgeom.rlib
$RC --crate-type rlib -O -L . --extern geom=libgeom.rlib util.rs && ar t libutil.rlib
$RC -O $LK -L . --extern util=libutil.rlib --print link-args app.rs -o app | args | grep -nE 'rcgu|rlib|"-l|-B|as-needed'
run_x86 ./app
echo "######## panic=unwind 与 panic=abort"
cd ../panic
for P in unwind abort; do
  $RC -O -C panic=$P -C codegen-units=1 --emit obj work.rs -o work_$P.o
  echo "== work_$P.o"; size -A work_$P.o | awk '/^\.text/ {t+=$2} /eh_frame/ {e+=$2} /gcc_except/ {g+=$2} END {print "text " t ", .eh_frame " e ", .gcc_except_table " g+0}'
  $NM work_$P.o | awk '/drop_in_place/ {n++} END {print n+0}'
  $RC -O $LK -C panic=$P hello.rs -o hello_$P && strip -o hello_${P}_s hello_$P
  $RC -O $LK -C panic=$P --print link-args hello.rs -o hello_tmp | args | grep -o 'libpanic_[a-z]*'
done
size hello_unwind_s hello_abort_s
for f in hello_unwind_s hello_abort_s; do $RE -SW $f | grep -E 'eh_frame|gcc_except'; done
run_x86 sh -c './hello_unwind; ./hello_abort'
echo "######## no_std：自己写 _start"
cd ../nostd
$RC -O -C panic=abort $LK -C link-self-contained=no -C link-arg=-nostartfiles tiny.rs -o tiny_musl
file tiny_musl; size tiny_musl; $NM tiny_musl
run_x86 sh -c './tiny_musl; echo exit=$?'
echo "== 去掉 -nostartfiles"
$RC -O -C panic=abort $LK -C link-self-contained=no tiny.rs -o bad1 2>&1 | grep -oE "multiple definition of \`_start'|undefined reference to \`[a-z_]*'" | sort -u
echo "== 默认的 self-contained 模式加 -nostartfiles"
$RC -O -C panic=abort $LK -C link-arg=-nostartfiles tiny.rs -o bad2 2>&1 | grep -oE "multiple definition of \`_start'" | sort -u
echo "== 不加 panic=abort"
$RC -O $LK -C link-self-contained=no -C link-arg=-nostartfiles tiny.rs -o bad3 2>&1 | grep error | head -1
echo "== x86_64-unknown-none"
if rustup target list --installed | grep -q x86_64-unknown-none; then
  rustc --print cfg --target x86_64-unknown-none | grep panic
  rustc --edition 2024 --target x86_64-unknown-none -O --print link-args tiny.rs -o tiny_none | args | tr '\n' ' '; echo
  file tiny_none; size tiny_none; $RE -rW tiny_none
  S2=$(rustc --print sysroot)/lib/rustlib/x86_64-unknown-none/lib
  printf 'x86_64-unknown-none 的 libcompiler_builtins：%s 个成员，其中 .rcgu.o %s 个\n' \
    "$(ar t $S2/libcompiler_builtins-*.rlib | wc -l | tr -d ' ')" "$(ar t $S2/libcompiler_builtins-*.rlib | grep -c rcgu)"
  run_x86 sh -c './tiny_none; echo exit=$?'
fi
echo "== 函数指针表留下的 R_X86_64_RELATIVE 没人处理"
if [ -f tiny_none ]; then
  rustc --edition 2024 --target x86_64-unknown-none -O tiny_tbl.rs -o tiny_tbl && $RE -rW tiny_tbl
  $OD -s -j .data.rel.ro tiny_tbl | tail -1; $NM tiny_tbl | grep write
  run_x86 sh -c './tiny_tbl; echo exit=$?'
fi
exit $FAIL
