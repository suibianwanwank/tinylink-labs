#!/usr/bin/env bash
# 重新生成 tests/elf/fixtures 下的目标文件和 GNU binutils 的参考输出。
#
# 目标文件用 clang 交叉编译（与第 2 章相同的命令），predict 另用 GNU as 汇编一份，
# 因为两个汇编器的产物在节名字符串表等细节上不同。参考输出来自 GNU readelf 和 nm，
# 测试会拿 tinylink 的输出和它们比较。生成结果提交进仓库，测试本身不依赖这些工具。
set -euo pipefail
cd "$(dirname "$0")"

clang=${CLANG:-clang}
target=--target=x86_64-unknown-linux-gnu
readelf=${READELF:-x86_64-linux-musl-readelf}
nm=${NM:-x86_64-linux-musl-nm}

$clang $target -O1 -c src/main.c -o main.o
$clang $target -O1 -c src/add.c -o add.o
$clang $target -O1 -c src/bind.c -o bind.o
$clang $target -O0 -c src/quiz.c -o quiz.o
$clang $target -c src/predict.s -o predict.o
x86_64-linux-musl-as src/predict.s -o predict-gas.o
$clang $target -c src/exit42.s -o exit42.o
$clang $target -c src/exit42-reloc.s -o exit42-reloc.o
$clang --target=aarch64-unknown-linux-gnu -c src/exit42-a64.s -o exit42-a64.o
$clang $target -fcommon -c src/common.c -o common.o

# 两个真正的链接器产出的可执行文件，用来对拍 readelf -l 的段和"节到段的映射"。
ld.lld -static exit42.o -o exit42-lld.elf
x86_64-linux-musl-ld -static exit42.o -o exit42-gnu.elf

mkdir -p expected
for o in *.o; do
    base=${o%.o}
    # macOS 的文件系统默认不区分大小写，-S 和 -s 的输出不能只靠大小写区分文件名。
    $readelf -W -h "$o" > "expected/$base.header"
    $readelf -W -S "$o" > "expected/$base.sections"
    $readelf -W -s "$o" > "expected/$base.symbols"
    $readelf -W -r "$o" > "expected/$base.relocs"
    $readelf -W -l "$o" > "expected/$base.segments"
    # nm 按名字排序时遵循当前 locale；固定成 C locale，按字节比较，结果才可复现。
    LC_ALL=C $nm "$o" > "expected/$base.nm"
done

for e in *.elf; do
    base=${e%.elf}
    $readelf -W -h "$e" > "expected/$base.header"
    $readelf -W -l "$e" > "expected/$base.segments"
    $readelf -W -S "$e" > "expected/$base.sections"
    $readelf -W -s "$e" > "expected/$base.symbols"
    LC_ALL=C $nm "$e" > "expected/$base.nm"
done

# 没有节名字符串表的可执行文件：把 ld.lld 产物的 e_shstrndx（偏移 0x3e）改成 0。
python3 -c '
data = bytearray(open("exit42-lld.elf", "rb").read())
data[0x3e:0x40] = bytes(2)
open("exit42-lld.nostr", "wb").write(data)
'
$readelf -W -l exit42-lld.nostr > expected/exit42-lld.nostr.segments
$readelf -W -S exit42-lld.nostr > expected/exit42-lld.nostr.sections

# M2：用 tinylink 自己链接两个 exit42，产物作为字节级的回归快照，再用 GNU readelf 读一遍。
tinylink=../../../target/debug/tinylink
(cd ../../.. && cargo build -q)
for base in exit42 exit42-a64; do
    $tinylink link -o "expected/$base.exe" "$base.o"
    $readelf -W -h "expected/$base.exe" > "expected/$base.exe.header"
    $readelf -W -h -l "expected/$base.exe" > "expected/$base.exe.hl"
    $readelf -W -l "expected/$base.exe" > "expected/$base.exe.segments"
    $readelf -W -s "expected/$base.exe" > "expected/$base.exe.symbols"
done

# 链接错误目录里的输入。
errors=../../link/errors
mkdir -p "$errors"
cp exit42-reloc.o "$errors/has-relocations.o"
cp expected/exit42.exe "$errors/not-relocatable.o"
$clang $target -c src/no-start.s -o "$errors/no-start.o"
$clang $target -c src/writable.s -o "$errors/writable.o"

# 畸形输入：在 main.o 上改几个字节。偏移是对着 expected/main.header 和 main.sections 算的：
# 节头表在 0x2f0，每项 64 字节；.rela.text 在 0x1e8，每条 24 字节，r_info 在第 8 字节。
bad=../errors
mkdir -p "$bad"
python3 - "$bad" <<'PY'
import struct, sys
out = sys.argv[1]
main = open("main.o", "rb").read()

def patch(data, offset, fmt, value):
    b = bytearray(data)
    struct.pack_into(fmt, b, offset, value)
    return bytes(b)

cases = {
    "truncated": main[:0x200],                                    # 节头表被截掉
    "bad-shoff": patch(main, 0x28, "<Q", 0xffff_ffff_ffff_0000),  # e_shoff 指到文件外
    "bad-shname": patch(main, 0x2f0 + 2 * 64, "<I", 0x10000),     # 2 号节的 sh_name 越过字符串表
    "bad-symbol-index": patch(main, 0x1e8 + 12, "<I", 99),        # 第一条重定位指向 99 号符号
    "not-elf": b"hello, linker\n",
}
for name, data in cases.items():
    open(f"{out}/{name}.o", "wb").write(data)
PY
