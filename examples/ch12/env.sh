# Linux x86-64 原生编译、链接和运行。
[ "$(uname -s)" = Linux ] && [ "$(uname -m)" = x86_64 ] || { echo 'requires x86-64 Linux' >&2; exit 2; }
export LC_ALL=C
ulimit -c 0
if [ -z "${W:-}" ]; then W=$(mktemp -d "${TMPDIR:-/tmp}/linker-ch12.XXXXXX"); export W; fi
SR=/usr/lib/x86_64-linux-musl
CC="clang -fno-pie"
link_musl() { out=$1; shift; ld.lld -static -o "$out" "$SR/crt1.o" "$SR/crti.o" "$@" -L"$SR" -lc "$SR/crtn.o"; }
run_x86() { (cd "$W"; timeout --kill-after=2s 120s "$@"); }
link_musl_dyn() { out=$1; shift; ld.lld -pie -dynamic-linker /lib/ld-musl-x86_64.so.1 -o "$out" "$SR/Scrt1.o" "$SR/crti.o" "$@" -L"$SR" -lc "$SR/crtn.o"; }
