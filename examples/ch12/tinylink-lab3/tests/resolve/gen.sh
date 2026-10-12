#!/usr/bin/env bash
# 重新生成 tests/resolve 下的目标文件，以及两个真实链接器对每个用例的结论。
#
# 用例是 golden/ 和 errors/ 下的 NAME.expect，第二行 `# args:` 列出输入文件。
# 输入文件按名字从 src/ 编译，放在用例所在的目录里：
#   X-common.o  <- src/X.c，加 -fcommon（没有初值的全局变量成为 COMMON 符号）
#   X.o         <- src/X.c，加 -fno-common（clang 11 起的默认值），或者 src/X.s
#   a64.o       <- src/a64.s，AArch64
#   not-elf.o   <- 一个文本文件
#
# 然后对每个 `tinylink resolve` 用例，用 ld.lld 和 GNU ld 链接同一组输入，由 verdict.py 把结论写进
# linkers/NAME.lld 和 linkers/NAME.gnu。tests/resolve_vs_linkers.rs 拿 tinylink 的结论和它们比较。
# 第三行写了 `# linkers: skip (...)` 的用例不比较。
set -euo pipefail
cd "$(dirname "$0")"

clang=${CLANG:-clang}
target=--target=x86_64-unknown-linux-gnu
lld=${LLD:-ld.lld}
gnuld=${GNULD:-x86_64-linux-musl-ld}
readelf=${READELF:-x86_64-linux-musl-readelf}

build() {
    local dir=$1 obj=$2 base=${2%.o}
    case $obj in
        not-elf.o) printf 'hello, linker\n' > "$dir/$obj" ;;
        a64.o) $clang --target=aarch64-unknown-linux-gnu -c src/a64.s -o "$dir/$obj" ;;
        *-common.o) $clang $target -O1 -fcommon -c "src/${base%-common}.c" -o "$dir/$obj" ;;
        *)
            if [[ -f src/$base.c ]]; then
                $clang $target -O1 -fno-common -c "src/$base.c" -o "$dir/$obj"
            else
                $clang $target -c "src/$base.s" -o "$dir/$obj"
            fi
            ;;
    esac
}

for expect in golden/*.expect errors/*.expect; do
    dir=$(dirname "$expect")
    for word in $(sed -n '2s/^# args://p' "$expect"); do
        [[ $word == *.o ]] && build "$dir" "$word"
    done
done

rm -rf linkers
mkdir -p linkers
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
for expect in golden/*.expect errors/*.expect; do
    name=$(basename "$expect" .expect)
    dir=$(dirname "$expect")
    args=$(sed -n '2s/^# args://p' "$expect")
    [[ $args == " resolve "* ]] || continue
    grep -q '^# linkers: skip' "$expect" && continue
    objs=${args# resolve }
    for which in lld gnu; do
        rm -rf "${work:?}"/*
        for o in $objs; do cp "$dir/$o" "$work/"; done
        if [[ $which == lld ]]; then ld=$lld; else ld=$gnuld; fi
        status=0
        (cd "$work" && $ld -Map=out.map -o out.elf $objs) > "$work/stderr" 2>&1 || status=$?
        python3 verdict.py "$work" "$which" "$status" "$readelf" $objs > "linkers/$name.$which"
        # lld 的报错原样留一份（去掉入口的警告），tests/resolve_vs_linkers.rs 拿 tinylink 的 stderr 和它逐字节比较。
        if [[ $which == lld && $status != 0 ]]; then
            grep -v 'warning: cannot find entry symbol' "$work/stderr" | sed 's/^ld\.lld:/tinylink:/' \
                > "linkers/$name.lld-stderr"
        fi
    done
done
