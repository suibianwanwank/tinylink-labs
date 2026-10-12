# 在 x86-64 Linux 上原生编译和执行。
[ "$(uname -s)" = Linux ] && [ "$(uname -m)" = x86_64 ] || { echo 'requires x86-64 Linux' >&2; exit 2; }
export LC_ALL=C
ulimit -c 0
if [ -z "${W:-}" ]; then W=$(mktemp -d "${TMPDIR:-/tmp}/linker-ch13.XXXXXX"); export W; fi
run_deb() { (cd "$W"; "$@"); }
run_abi() { (cd "$W"; "$@"); }
stage() { d=$1; mkdir -p "$W/$d"; cp -R "$(dirname "$0")"/. "$W/$d/"; }
