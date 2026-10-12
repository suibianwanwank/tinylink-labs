#!/usr/bin/env bash
# 在 Linux 上构造和解析 Mach-O；这些格式样本不在 Linux 上执行。
set -euo pipefail
[[ $(uname -s) == Linux && $(uname -m) == x86_64 ]]
here=$(cd "$(dirname "$0")" && pwd)
work=$(mktemp -d "${TMPDIR:-/tmp}/machoinfo-linux.XXXXXX")
echo "format fixtures: $work"
cd "$work"
printf 'int main(void) { return 42; }\n' > prog.c
printf 'int answer(void) { return 42; }\n' > liba.c
printf 'extern int answer(void); int main(void) { return answer(); }\n' > main.c
for arch in x86_64 arm64; do
    for source in prog main liba; do
        clang --target="$arch-apple-macos14" -O1 -fno-stack-protector -c "$source.c" -o "$source-$arch.o"
    done
    ld64.lld -arch "$arch" -platform_version macos 14.0 14.0 -e _main -o "prog-$arch" "prog-$arch.o"
    ld64.lld -arch "$arch" -platform_version macos 14.0 14.0 -dylib \
        -install_name @rpath/liba.dylib -o "liba-$arch.dylib" "liba-$arch.o"
    # No SDK/runtime is needed for parsing. Dynamic lookup permits the stub binder reference.
    ld64.lld -arch "$arch" -platform_version macos 14.0 14.0 -e _main \
        -undefined dynamic_lookup -rpath @executable_path -o "main-$arch" \
        "main-$arch.o" "liba-$arch.dylib"
done
llvm-lipo -create prog-x86_64 prog-arm64 -output prog-fat
sh "$here/check.sh" prog-fat main-x86_64 main-arm64 liba-x86_64.dylib liba-arm64.dylib
"$here/target/release/machoinfo" prog-fat > prog-fat.info
cat prog-fat.info
# Keep corruption deterministic: truncate inside the load-command region.
head -c 64 prog-x86_64 > truncated
status=0
"$here/target/release/machoinfo" truncated > truncated.out 2>truncated.err || status=$?
[[ $status == 1 ]]
grep 'runs past end of file' truncated.err
printf 'truncated Mach-O rejected with exit %s\n' "$status"
echo 'Linux Mach-O format checks passed; no Mach-O executable was run'
