#!/bin/sh
# Run from any directory on x86-64 Linux; retain artifacts for inspection.
set -eu
ulimit -c 0
[ "$(uname -s)" = Linux ] && [ "$(uname -m)" = x86_64 ]
here=$(cd "$(dirname "$0")" && pwd)
lab=$(mktemp -d "${TMPDIR:-/tmp}/linker-ch01.XXXXXX")
echo "experiment directory: $lab"
cp "$here"/*.c "$here"/*.cpp "$here"/*.s "$lab/"
cd "$lab"
clang --version | head -1
gcc --version | head -1
ld.bfd --version | head -1
clang -O1 -c main.c -o main-clang.o
objdump -t -d -r main-clang.o
# Resolve musl's library through its actual link search directory. GCC's
# -print-file-name can select glibc when used through a specs-only wrapper.
musl_lib=$(musl-gcc -print-file-name=libc.a)
echo "driver library query: $musl_lib"
musl-gcc -static hello42.c -Wl,-t,-Map=hello42.map -o hello42 > link-inputs.txt
cat link-inputs.txt
musl_lib=$(awk '/\/libc\.a$/ {print; exit}' link-inputs.txt)
[ -n "$musl_lib" ] && cp "$musl_lib" libc.a
file libc.a
ar t libc.a | head -5
ar t libc.a | wc -l
printf_member=$(ar t libc.a | awk '/^printf\.(o|lo)$/ {print; exit}')
[ -n "$printf_member" ]
ar x libc.a "$printf_member"
[ -f "$printf_member" ]
file "$printf_member"
sed -n '/^Archive member included/,/^Merging/p' hello42.map
readelf -h hello42
gcc -O1 -c main.c -o main.o
gcc -O1 -g -c main.c -o main-g.o
g++ -O1 -c words.cpp -o words.o
g++ -O1 -g -c words.cpp -o words-g.o
wc -c main.o main-g.o words.o words-g.o
size main.o main-g.o words.o words-g.o
objdump -h words-g.o
gcc -c answer.s use.c
musl-gcc -static use.o answer.o -o bad
musl-gcc -static -fuse-ld=lld -Wl,--no-dynamic-linker use.o answer.o -o bad-lld
readelf -lW bad | grep GNU_STACK
readelf -lW bad-lld | grep GNU_STACK
printf '\n.section .note.GNU-stack,"",@progbits\n' >> answer.s
gcc -c answer.s
musl-gcc -static use.o answer.o -o good
readelf -lW good | grep GNU_STACK
status=0
timeout 5s ./good || status=$?
[ "$status" = 42 ]
echo "good exit: $status"
