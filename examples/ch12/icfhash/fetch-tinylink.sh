#!/bin/sh
# 源码快照与本章一同分发；仅当显式指定 TINYLINK 时重新导出 lab3。
set -e
HERE=$(cd "$(dirname "$0")" && pwd)
DST="$HERE/../tinylink-lab3"
if [ -n "${TINYLINK:-}" ]; then mkdir -p "$DST"; git -C "$TINYLINK" archive lab3 | tar -x -C "$DST"; fi
[ -f "$DST/Cargo.toml" ] || { echo "missing bundled tinylink-lab3" >&2; exit 1; }
