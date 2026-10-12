#!/bin/sh
# 用 otool -l 对拍 machoinfo：比较每个架构的 segment（名字、vmaddr、vmsize、fileoff、filesize、
# maxprot、initprot）、section 名单和依赖的 dylib、rpath。
set -e
OTOOL=${OTOOL:-llvm-otool}
LIPO=${LIPO:-llvm-lipo}
work=$(mktemp -d "${TMPDIR:-/tmp}/machoinfo-check.XXXXXX")
trap 'rm -rf "$work"' EXIT
D=$(cd "$(dirname "$0")" && pwd)
cargo build --release --quiet --manifest-path "$D/Cargo.toml"
BIN="$D/target/release/machoinfo"
fail=0
for f in "$@"; do
  for arch in $("$LIPO" -archs "$f"); do
    # 先抽取薄片，避免不同 otool 对 fat 文件的 -arch 行为差异。
    reference=$f
    if [ "$("$LIPO" -archs "$f" | wc -w)" -gt 1 ]; then
      "$LIPO" "$f" -thin "$arch" -output "$work/thin"
      reference=$work/thin
    fi
    # otool 一侧：段名、节名（segname,sectname）、dylib 路径
    "$OTOOL" -l "$reference" | awk '
      /^ *cmd /{cmd=$2}
      cmd=="LC_SEGMENT_64" && /^ *segname/ && !insect {seg=$2}
      cmd=="LC_SEGMENT_64" && !insect && /^ *(vmaddr|vmsize|fileoff|filesize|maxprot)/ {v[$1]=$2}
      cmd=="LC_SEGMENT_64" && !insect && /^ *initprot/ {print "segment " seg, v["vmaddr"], v["vmsize"], v["fileoff"], v["filesize"], v["maxprot"], $2}
      /^Section/{insect=1}
      /^Load command/{insect=0}
      insect && /^ *sectname/{s=$2}
      insect && /^ *segname/{print "section " $2 "," s}
      (cmd=="LC_LOAD_DYLIB"||cmd=="LC_LOAD_WEAK_DYLIB"||cmd=="LC_REEXPORT_DYLIB"||cmd=="LC_ID_DYLIB") && /^ *name /{print "dylib " $2}
      cmd=="LC_RPATH" && /^ *path /{print "rpath " $2}' > "$work/reference.txt"
    # machoinfo 一侧：取出对应架构的那一片
    "$BIN" "$f" | awk -v a="$arch" '
      /^## fat slice/{next}
      /^arch /{cur=$2; next}
      cur!=a {next}
      /^segment /{print "segment " $2, $4, $6, $8, $10, $12, $14}
      /^  section /{gsub(/ +,/, ","); print "section " $2}
      /^dylib /{print "dylib " $3}
      /^rpath /{print "rpath " $2}' > "$work/actual.txt"
    n=$(wc -l < "$work/actual.txt" | tr -d ' ')
    if diff -u "$work/reference.txt" "$work/actual.txt" > /dev/null; then
      echo "OK   $f [$arch] $n 行一致"
    else
      echo "DIFF $f [$arch]"; diff -u "$work/reference.txt" "$work/actual.txt" | head -n 20; fail=1
    fi
  done
done
exit $fail
