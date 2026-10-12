#!/bin/sh
# Run already-built baseline programs on Linux. This does not claim native CPU execution.
set -eu
[ "$#" -eq 1 ] || { echo "usage: sh check.sh DIRECTORY" >&2; exit 125; }
[ "$(uname -s)" = Linux ] || { echo "check: Linux required" >&2; exit 125; }
case $1 in /*) baseline_dir=$1 ;; *) baseline_dir=$PWD/$1 ;; esac
command -v timeout >/dev/null
ulimit -c 0
check_exit() {
    expected=$1
    shift
    actual=0
    "$@" || actual=$?
    printf 'expected=%s actual=%s\n' "$expected" "$actual"
    [ "$actual" -eq "$expected" ]
}
check_exit 42 timeout --kill-after=1s 10s "$baseline_dir/hello" 'two words'
check_exit 139 timeout --kill-after=1s 10s "$baseline_dir/crash"
check_exit 124 timeout --kill-after=1s 1s "$baseline_dir/spin"
echo 'Linux execution checks passed'
