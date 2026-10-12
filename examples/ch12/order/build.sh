#!/bin/sh
# 节顺序实验：Linux x86-64 原生 Clang/LLD + glibc。
set -e
. "$(dirname "$0")/../env.sh"
N=${N:-8192}; STRIDE=${STRIDE:-16}; ITER=${ITER:-20000}
D="$W/order$TAG"; mkdir -p "$D"; cp "$(dirname "$0")"/gen.py "$(dirname "$0")"/pages.py "$D/"; cd "$D"
python3 gen.py $N $STRIDE > prog.c
clang -O2 -fno-pie -ffunction-sections -DITER=$ITER -c prog.c -o prog.o
clang -fuse-ld=lld -static -no-pie prog.o -o p_default
clang -fuse-ld=lld -static -no-pie prog.o -Wl,--symbol-ordering-file=hot.txt -o p_ordered
python3 pages.py p_default p_ordered
