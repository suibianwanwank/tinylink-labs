#!/bin/sh
# Linux x86-64：g++/clang++、binutils/LLVM、musl-gcc、Rust 与 musl/none 目标。
set -u
W=$(mktemp -d "${TMPDIR:-/tmp}/linker-ch14.XXXXXX"); export W
printf 'experiment directory: %s\n' "$W"
D=$(cd "$(dirname "$0")" && pwd)
failed=""
for s in puzzle/run.sh mangle/run.sh mangle/run2.sh vague/run.sh vague/odr.sh vague/key.sh \
         init/run.sh guard/run.sh rust/run.sh ex/run.sh demangle/run.sh; do
  echo "######## $s"
  sh "$D/$s" || failed="$failed $s"
done
if [ -n "$failed" ]; then echo "!! 失败的脚本：$failed" >&2; exit 1; fi
echo "全部通过"
