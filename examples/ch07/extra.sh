#!/bin/sh
set -u
ulimit -c 0
cd "$1"
printf '\n== copy ==\n'
mkdir copy; cp vec.c main.c copy; cd copy
gcc -fPIC -O1 -shared -Wl,-soname,libvec.so.1 vec.c -o libvec.so.1
ln -s libvec.so.1 libvec.so
gcc -fno-pic -no-pie main.c -L. -lvec '-Wl,-rpath,$ORIGIN' -o main_nopie
readelf -rW main_nopie
./main_nopie; echo exit=$?
sed 's/int counter;/long counter;/' vec.c > vec8.c
gcc -fPIC -O1 -shared -Wl,-soname,libvec.so.1 vec8.c -o libvec.so.1
./main_nopie; echo exit=$?
cd ..
printf '\n== preload ==\n'
printf 'int helper(int x) { return 100; }\n' > fake.c
musl-gcc -shared -fPIC fake.c -o libfake.so
LD_PRELOAD=./libfake.so ./main_pie; echo exit=$?
printf '\n== bad allocator bindings ==\n'
LD_PRELOAD=./ex3-gcc/libfast.so LD_DEBUG=bindings ./ex3-gcc/app 2>&1 | grep -E 'symbol .(malloc|free|strdup).'
printf '\n== native missing lib/symbol/version ==\n'
cd ex1
mv libvec.so.1 saved.so
./main; echo exit=$?
printf 'int counter; int helper(int x){return x;}\n' > incomplete.c
gcc -shared -fPIC incomplete.c -Wl,-soname,libvec.so.1 -o libvec.so.1
./main; echo exit=$?
printf 'VEC_2.0 { global: bump; counter; local: *; };\n' > vec.map
gcc -shared -fPIC vec.c -Wl,-soname,libvec.so.1 -Wl,--version-script=vec.map -o libvec.so.1
gcc main.c -L. -lvec '-Wl,-rpath,$ORIGIN' -o main_ver
sed -i 's/VEC_2.0/VEC_1.0/' vec.map
gcc -shared -fPIC vec.c -Wl,-soname,libvec.so.1 -Wl,--version-script=vec.map -o libvec.so.1
./main_ver; echo exit=$?
mv saved.so libvec.so.1

cd "$1"
readelf -lW main_pie
cat > fast.s <<'EOF'
.text
.globl fast
.type fast,@function
fast: ret
EOF
as fast.s -o fast.o
ld -shared fast.o vec.o -o libmix.so
ld.lld -shared fast.o vec.o -o libmix_lld.so
readelf -lW libmix.so | grep GNU_STACK
readelf -lW libmix_lld.so | grep GNU_STACK
ld -shared fast.o -o fast.so
readelf -lW fast.so | grep GNU_STACK
cat > ifunc.c <<'EOF'
static int impl(void) { return 7; }
static int (*resolve(void))(void) { return impl; }
int pick(void) __attribute__((ifunc("resolve")));
EOF
clang -c ifunc.c -o ifunc.o
readelf -h ifunc.o | grep OS/ABI
file ifunc.o
