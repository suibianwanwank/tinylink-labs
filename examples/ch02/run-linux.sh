#!/bin/sh
set -eu
[ "$(uname -sm)" = "Linux x86_64" ] || exit 1
ulimit -c 0
D=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
W=$(mktemp -d "${TMPDIR:-/tmp}/linker-ch02.XXXXXX")
cp "$D"/*.c "$D"/*.s "$D"/*.S "$D"/*.py "$W/"
cd "$W"
echo "WORKDIR=$W"
run() { printf '\n$ %s\n' "$*"; sh -c "$*"; }
run 'clang -O1 -ccc-print-phases -c add.c; clang -O1 -save-temps -c add.c; cat add.s'
run 'clang -c hand.S -o hand-small.o; llvm-objdump -s -j .rodata hand-small.o; clang -DBIG -c hand.S -o hand-big.o; llvm-objdump -s -j .rodata hand-big.o'
run 'cp hand.S plain.s; clang -DBIG -c plain.s -o plain.o; echo status=$?'
run 'clang -x assembler-with-cpp -DBIG -c plain.s -o plain.o; llvm-objdump -s -j .rodata plain.o'
run 'clang -O1 -c calls.c -o calls.o; llvm-objdump -dr calls.o; clang -O1 -ffunction-sections -c calls.c -o calls-fs.o; llvm-objdump -dr calls-fs.o'
run 'clang -O1 -c main.c add.c; file main.o add.o; xxd -l 64 main.o; wc -c main.o; llvm-objdump -fph main.o; python3 shdr.py main.o; llvm-objdump -s -j .strtab -j .data -j .comment -j .symtab -j .rela.text main.o; llvm-objdump -tdr main.o; nm main.o'
run 'clang -O1 -c bind.c -o bind.o; llvm-objdump -tdr bind.o; llvm-objdump -t hand-small.o'
run 'clang -c start.S -o start.o; ld.lld -o prog start.o main.o add.o; readelf -lW prog; ./prog; echo exit=$?'
run 'clang -O0 -c quiz.c -o quiz.o; readelf -sW quiz.o; clang -O1 -c quiz.c -o quiz1.o; readelf -sW quiz1.o'
run 'clang -c predict.s -o predict.o; readelf -sW predict.o; llvm-objdump -dr predict.o; llvm-objdump -s -j .data predict.o; as predict.s -o predict-gas.o; llvm-objdump -dr predict-gas.o'
for flag in ax a x; do
 sed "s/\"ax\"/\"$flag\"/" flag.s > flag-$flag.s
 run "clang -c flag-$flag.s -o flag-$flag.o; ld.lld -o lld-$flag flag-$flag.o; ld -o gnu-$flag flag-$flag.o; readelf -lW lld-$flag; readelf -lW gnu-$flag; ./lld-$flag; echo lld_exit=\$?; ./gnu-$flag; echo gnu_exit=\$?"
done
echo 'completed Linux native ch02'
