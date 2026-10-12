#!/bin/sh
# 练习四 (1)(2)：把 tinylink 的 lab2 导出到临时目录，加上 COMPRESSED 常量和 debugsec 示例后运行。
# 不改动 tinylink 仓库本身。用法：sh run.sh   （先跑 ../tomb/run.sh）
set -e
D=$(cd "$(dirname "$0")" && pwd)
: "${TL:?请将 TL 设为本机 tinylink 仓库路径}"
W=${W:-$HOME/.cache/linker-lab/ch11}
T=$(mktemp -d /tmp/tl-lab2.XXXXXX)
git -C "$TL" archive lab2 | tar -x -C "$T"
patch -s "$T/src/elf/types.rs" < "$D/types.rs.diff"
mkdir -p "$T/examples" && cp "$D/debugsec.rs" "$T/examples/"
(cd "$T" && cargo build -j2 -q --example debugsec)
for o in gc_clang.o gc_gcc.o gc_gcc_z.o; do
    echo "== $o"; "$T/target/debug/examples/debugsec" "$W/tomb/$o"
done
find "$T" -mindepth 1 -delete && rmdir "$T"
