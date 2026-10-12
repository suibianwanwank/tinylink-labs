#!/bin/sh
# 把开头的谜题换成 -flto=thin：摘要、导入，以及 scale 为什么留了下来。先运行 ../puzzle/run.sh。
set -e
. "$(dirname "$0")/../env.sh"
cd "$W/puzzle"
set -x
$CC -O2 -flto=thin -c main.c -o main.thin.o
$CC -O2 -flto=thin -c util.c -o util.thin.o
mkdir -p th && find th -mindepth 1 -delete && cp main.thin.o util.thin.o th/ && cd th
link_musl thin main.thin.o util.thin.o --save-temps
ls
llvm-bcanalyzer -dump main.thin.o | sed -n '/<GLOBALVAL_SUMMARY_BLOCK/,/<\/GLOBALVAL_SUMMARY_BLOCK/p'
cat thin.index.dot
llvm-dis main.thin.o.3.import.bc -o - | grep -E '^define|^declare'
llvm-dis util.thin.o.2.internalize.bc -o - | grep -E '^define|^declare'
llvm-nm thin | grep -E ' (main|scale|twice|checksum)$'
