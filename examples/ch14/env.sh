# 在 x86-64 Linux 中编译、链接、直接执行；所有产物进入独立实验目录。
[ "$(uname -s)" = Linux ] && [ "$(uname -m)" = x86_64 ] || { echo "requires x86-64 Linux" >&2; exit 2; }
export LC_ALL=C
ulimit -c 0
if [ -z "${W:-}" ]; then W=$(mktemp -d "${TMPDIR:-/tmp}/linker-ch14.XXXXXX"); export W; fi
CXX_CLANG="clang++ -fno-pie"
GXX="g++ -fno-pie"
NM=nm
RE=readelf
OD=objdump
FAIL=0
ok() { "$@"; rc=$?; [ $rc -eq 0 ] || { echo "!! FAILED (rc=$rc): $*" >&2; FAIL=1; }; return $rc; }
run_x86() { ok timeout --kill-after=2s 20s "$@"; }
run_native() { ok timeout --kill-after=2s 20s "$@"; }
stage() {
 d=$1; mkdir -p "$W/$d"
 cp -R "$(dirname "$0")"/. "$W/$d/"; cd "$W/$d" || exit 1
}
