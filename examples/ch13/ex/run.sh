#!/bin/sh
# 第 13 章练习一到三。练习四见 ../verdump/run.sh。
set -e
. "$(dirname "$0")/../env.sh"
stage ex
cp -R "$(dirname "$0")/../symver" "$W/" 2>/dev/null || true
echo "######## 练习一"; run_abi bash $W/ex/steps1.sh
echo "######## 练习二"; run_abi sh -c 'cd ex && sh steps2.sh'
echo "######## 练习三"; run_deb sh -c 'cd ex && sh steps3.sh'
