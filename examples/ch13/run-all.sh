#!/bin/sh
# Linux 全章重跑，独立目录保存所有输入和输出。
W=$(mktemp -d "${TMPDIR:-/tmp}/linker-ch13.XXXXXX"); export W
printf 'experiment directory: %s\n' "$W"
D=$(cd "$(dirname "$0")" && pwd)
failed=""
for s in break/run.sh table/run.sh soname/run.sh symver/run.sh glibc/run.sh abi/run.sh \
         ex/run.sh verdump/run.sh; do
  echo "######## $s"
  sh "$D/$s" || { echo "ERROR: $s 失败" >&2; failed="$failed $s"; }
done
if [ -n "$failed" ]; then
  echo "以下步骤未完成：$failed" >&2
  exit 1
fi
echo "全部步骤完成"
