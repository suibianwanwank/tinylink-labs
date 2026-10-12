#!/bin/sh
# Compare input declarations, linker policy, and explicit output policy.
set -eu
[ "$(uname -s)" = Linux ] && [ "$(uname -m)" = x86_64 ] || {
    echo 'requires native x86-64 Linux' >&2
    exit 2
}
here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
work=$(mktemp -d "${TMPDIR:-/tmp}/linker-stack-policy.XXXXXX")
trap 'rm -rf "$work"' EXIT HUP INT TERM
cp "$here/lib.c" "$here/fast_add.s" "$work/"
cd "$work"
clang --version | head -n 1
ld --version | head -n 1
ld.lld --version
clang -O2 -fPIC -c lib.c -o lib.o
clang -c fast_add.s -o fast_add.o
for linker in ld ld.lld; do
    for mode in mixed absent explicit-noexec explicit-exec; do
        case "$mode" in
            mixed) set -- lib.o fast_add.o ;;
            absent) set -- fast_add.o ;;
            explicit-noexec) set -- -z noexecstack lib.o fast_add.o ;;
            explicit-exec) set -- -z execstack lib.o fast_add.o ;;
        esac
        output="$linker-$mode.so"
        "$linker" -shared "$@" -o "$output"
        echo "$linker: $mode"
        readelf -lW "$output" | awk '
            /GNU_STACK/ { print; found=1 }
            END { if (!found) print "  no PT_GNU_STACK" }
        '
    done
done
