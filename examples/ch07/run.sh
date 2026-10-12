#!/bin/sh
# x86-64 Linux 本机编译、链接、运行，分别测 glibc 与 musl。
set -eu
[ "$(uname -sm)" = "Linux x86_64" ] || exit 1
ulimit -c 0
D=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
W=$(mktemp -d "${TMPDIR:-/tmp}/linker-ch07.XXXXXX")
cp "$D"/*.c "$D"/*.h "$D"/*.s "$D"/use.map "$W/"
cd "$W"
echo "WORKDIR=$W"
run() { printf '\n$ %s\n' "$*"; sh -c "$*"; }
run 'gcc --version | head -1; clang --version | head -1; ld --version | head -1; ldd --version | head -1; /lib/ld-musl-x86_64.so.1 2>&1 | head -3'
run 'clang -fPIC -O1 -c vec.c -o vec.o; ld.lld -shared -soname libvec.so.1 vec.o -o libvec.so.1; ln -s libvec.so.1 libvec.so; readelf -dWrW libvec.so.1; objdump -d libvec.so.1'
run 'clang -fPIE -O1 -c main.c -o main_pie.o; musl-gcc -pie main_pie.o -L. -lvec -Wl,-rpath,\$ORIGIN -o main_pie; readelf -dWrW main_pie; ./main_pie; echo exit=$?'
run 'clang -c gotref.s -o gotref.o; ld -shared -soname libvec.so.1 vec.o -o lv_bfd.so; ld.lld -shared vec.o gotref.o -o lg_lld.so; for f in lv_bfd.so lg_lld.so; do readelf -sW $f | grep GLOBAL_OFFSET; readelf -SW $f | grep -E "\.got.plt|\.dynamic "; done'
run 'gcc -fPIE -O1 -c relr.c -o relr.o; gcc -pie relr.o -o relr_plain; gcc -pie -Wl,-z,pack-relative-relocs relr.o -o relr_packed; readelf -SW relr_plain | grep "rel.\.dyn"; readelf -SW relr_packed | grep "rel.\.dyn"; readelf -rW relr_packed; ./relr_packed; echo glibc_exit=$?'
run 'musl-gcc -fPIE -O1 -Wl,-z,pack-relative-relocs relr.c -o relr_musl; readelf -rW relr_musl; ./relr_musl; echo musl_exit=$?'
cat > ip.sh <<'EOF'
set -e
CC=$1; o=out-$CC; mkdir -p "$o"
"$CC" -c greet.c -o $o/greet.o && ar rcs $o/libgreet.a $o/greet.o
# 编译期：-I. 让本地 malloc.h 生效
"$CC" -I. -c hello.c -o $o/hello_c.o
"$CC" -c mymalloc_c.c -o $o/mymalloc_c.o
"$CC" $o/hello_c.o $o/mymalloc_c.o -L$o -lgreet -o $o/hello_c
# 链接期：动态与静态各一份
"$CC" -c hello.c -o $o/hello.o
"$CC" -c mymalloc_l.c -o $o/mymalloc_l.o
"$CC" -Wl,--wrap=malloc,--wrap=free $o/hello.o $o/mymalloc_l.o -L$o -lgreet -o $o/hello_l
"$CC" -static -Wl,--wrap=malloc,--wrap=free $o/hello.o $o/mymalloc_l.o -L$o -lgreet -o $o/hello_ls
# 加载期
"$CC" $o/hello.o -L$o -lgreet -o $o/hello_r
"$CC" -fPIC -shared mymalloc_r.c -o $o/mymalloc.so -ldl
echo "compile-time:       $($o/hello_c 2>&1 >/dev/null)"
echo "link-time dynamic:  $($o/hello_l 2>&1 >/dev/null)"
echo "link-time static:   $($o/hello_ls 2>&1 >/dev/null)"
echo "load-time:          $(LD_PRELOAD=$o/mymalloc.so $o/hello_r 2>&1 >/dev/null)"
# 静态链接的程序忽略 LD_PRELOAD：计数行不出现
"$CC" -static $o/hello.o -L$o -lgreet -o $o/hello_s
echo "static + preload:   [$(LD_PRELOAD=$o/mymalloc.so $o/hello_s 2>&1 >/dev/null)]"
# 静态 --wrap 版本里每次 malloc 的调用者（用 addr2line 把返回地址翻译成函数名）
sed -e 's/n_malloc++;/n_malloc++; { char b[32]; write(2, b, snprintf(b, 32, "ra %p\\n", __builtin_return_address(0))); }/' mymalloc_l.c > $o/l_trace.c
"$CC" -I. -c $o/l_trace.c -o $o/l_trace.o
"$CC" -static -Wl,--wrap=malloc,--wrap=free $o/hello.o $o/greet.o $o/l_trace.o -o $o/hello_lt
for a in $($o/hello_lt 2>&1 >/dev/null | sed -n 's/^ra //p'); do addr2line -f -e $o/hello_lt $a | head -1; done | tr '\n' ' '; echo
# dlsym 递归探测：两个版本都应该什么也不打印
"$CC" -fPIC -shared probe.c -o $o/probe.so -ldl
"$CC" hello.c greet.c -o $o/hp
LD_PRELOAD=$o/probe.so $o/hp; echo "probe status=$?"
EOF
run 'sh ip.sh gcc'
run 'sh ip.sh musl-gcc'
run 'mkdir ex1; cp vec.c main.c ex1; cd ex1; gcc -O1 -fPIC -shared -Wl,-soname,libvec.so.1 vec.c -o libvec.so.1; ln -s libvec.so.1 libvec.so; gcc -O1 main.c -L. -lvec -Wl,-z,lazy -Wl,-rpath,\$ORIGIN -o main; readelf -d main | grep -E "NEEDED|RUNPATH"; readelf -SW main | grep -E " \.got|\.data "; readelf -r main; LD_DEBUG=bindings ./main 2>&1 | grep -E "symbol .(bump|helper|counter).|transferring"; LD_BIND_NOW=1 LD_DEBUG=bindings ./main 2>&1 | grep -E "symbol .(bump|helper|counter).|transferring"'
run 'gcc -fPIC -O1 -c cnt.c -o cnt_gcc.o; ld.lld -shared cnt_gcc.o -o libcnt_gcc.so; readelf -rW libcnt_gcc.so; ld.lld -shared --version-script=use.map cnt_gcc.o -o libcnt_v.so; readelf -rW libcnt_v.so; clang -fPIC -O1 -c cnt.c -o cnt_clang.o; ld.lld -shared cnt_clang.o -o libcnt_clang.so; readelf -rW libcnt_clang.so; objdump -d cnt_clang.o'
cat > ex3.sh <<'EOF'
CC=$1; o=ex3-$CC; mkdir -p "$o"
"$CC" -O1 -fPIC -shared fastmalloc.c -o $o/libfast.so
"$CC" -O1 -fPIC -shared fastmalloc2.c -o $o/libfix.so
"$CC" -O1 app.c -o $o/app
$o/app; echo "status=$?"
LD_PRELOAD=$o/libfast.so $o/app; echo "status=$?"
LD_PRELOAD=$o/libfix.so $o/app; echo "fixed status=$?"
EOF
run 'sh ex3.sh gcc'
run 'sh ex3.sh musl-gcc'
run 'musl-gcc -static-pie -fPIE -O1 sp.c -o sp; readelf -rW sp; nm sp | grep -wE "cursor|table"; ./sp; echo exit=$?'
run 'cd ex1; ldd ./main; LD_DEBUG=libs ./main 2>&1 | head -35; readelf -V ../relr_packed'
sh "$D/extra.sh" "$W"
python3 "$D/last.py" "$W"
echo 'completed Linux native ch07'
