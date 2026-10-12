#!/bin/sh
# 所有编译、链接和 GDB 检查在 x86-64 Linux 上执行；xv6 是 RISC-V 格式对照。
set -eu
[ "$(uname -s)" = Linux ] && [ "$(uname -m)" = x86_64 ] || exit 2
D=$(cd "$(dirname "$0")" && pwd)
W=$(mktemp -d "${TMPDIR:-/tmp}/linker-ch11.XXXXXX"); export W
printf 'experiment directory: %s\n' "$W"
sh "$D/tomb/run.sh"
sh "$D/xv6/build.sh"
sh "$D/xv6/analyze.sh"
sh "$D/gdb/run.sh"
sh "$D/tomb/gdb.sh"
CARGO_TARGET_DIR="$W/buildid-target" cargo build -j2 -q --release --manifest-path "$D/buildid/Cargo.toml"
for f in "$W/gdb/prog" "$W/xv6/kernel-zstd" "$W/gdb/prog.debug.orig" "$W/tomb/bid_fast" "$W/tomb/gc_gcc.o" "$W/tomb/gc_gcc_z.o"; do
 "$W/buildid-target/release/buildid" "$f"
done
# 实现练习需要单独提供配套仓库；不会隐式访问作者的个人目录。
if [ -n "${TL:-}" ]; then sh "$D/tinylink-ex/run.sh"; fi
