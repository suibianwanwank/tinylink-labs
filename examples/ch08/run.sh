#!/bin/sh
# 在 Linux x86-64 上运行结构、回溯和查表实验。
set -eu
D=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
sh "$D/structure-linux.sh"
python3 "$D/run-linux.py"
