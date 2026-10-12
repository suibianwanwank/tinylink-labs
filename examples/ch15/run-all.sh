#!/usr/bin/env bash
# Linux x86-64: native ELF execution; Mach-O/PE/Wasm are generated and inspected only.
set -euo pipefail
[[ $(uname -s) == Linux && $(uname -m) == x86_64 ]]
export LC_ALL=C
ulimit -c 0
D=$(cd "$(dirname "$0")" && pwd)
O=$(mktemp -d "${TMPDIR:-/tmp}/linker-ch15.XXXXXX")
echo "format experiment directory: $O"
cd "$O"
h() { printf '\n######## %s\n' "$*"; }
mc() { clang --target=arm64-apple-macos14 -O1 "$@"; }
ml() { ld64.lld -arch arm64 -platform_version macos 14.0 14.0 -syslibroot "$O/interface-root" -L. -lSystem "$@"; }
# A deliberately minimal link-time interface fixture, not an SDK or executable runtime.
cat > libSystem.tbd <<'TBD'
--- !tapi-tbd-v3
archs: [ arm64, x86_64 ]
platform: macosx
install-name: '/usr/lib/libSystem.B.dylib'
exports:
  - archs: [ arm64, x86_64 ]
    symbols: [ _printf, _puts, _malloc, _atoi, dyld_stub_binder ]
...
TBD
mkdir -p interface-root/usr/lib
cp libSystem.tbd interface-root/usr/lib/libSystem.B.tbd
h '1 object files'
clang -O1 -c "$D/open/main.c" -o main-elf.o
mc -c "$D/open/main.c" -o main-macho.o
clang --target=x86_64-pc-windows-msvc -O1 -c "$D/open/main.c" -o main-coff.obj
for f in main-elf.o main-macho.o main-coff.obj; do
 llvm-readobj --file-headers "$f"
 llvm-objdump -h -dr "$f"; llvm-nm "$f"
done
for t in x86_64-unknown-linux-gnu arm64-apple-macos14 x86_64-pc-windows-msvc; do
 clang --target="$t" -x c++ -O1 -c "$D/open/add.c" -o "add-$t.o"
 echo "== $t"; llvm-nm "add-$t.o"
done
clang -O1 "$D/open/main.c" "$D/open/add.c" -o prog-elf
./prog-elf
h '2 Mach-O skeleton'
mc -c "$D/open/add.c" -o add-macho.o
ml -e _main main-macho.o add-macho.o -o prog
llvm-otool -hv main-macho.o; llvm-otool -l main-macho.o
llvm-otool -hv prog; llvm-otool -l prog
llvm-nm -m prog
llvm-objdump --macho --link-opt-hints main-macho.o
h '3 relocations and LOH'
mc -c "$D/reloc/kinds.c" -o kinds.o
clang --target=aarch64-unknown-linux-gnu -O1 -c "$D/reloc/kinds.c" -o kinds-elf.o
clang --target=x86_64-apple-macos14 -O1 -c "$D/open/main.c" -o main-x86macho.o
for f in kinds.o kinds-elf.o main-x86macho.o; do llvm-objdump -dr "$f"; done
llvm-otool -r kinds.o; llvm-objdump -s -j __data kinds.o
llvm-objdump -r -j .data kinds-elf.o
mc -c "$D/reloc/data.c" -o data.o
mc -c "$D/reloc/main.c" -o rmain.o
ml -e _main rmain.o kinds.o data.o -o rapp
llvm-nm -m rapp; llvm-objdump -d rapp
mc -c "$D/open/main.c" -o main-noloh.o -mllvm -aarch64-enable-collect-loh=false
ml -e _main main-noloh.o add-macho.o -o prog-noloh
for f in prog prog-noloh; do echo "== $f"; llvm-objdump -d "$f"; done
h '4 two-level namespace and rpath'
for n in a b a2; do
 mc -c "$D/twolevel/$n.c" -o "lib$n.o"
 ml -dylib "lib$n.o" -install_name "@rpath/lib$n.dylib" -o "lib$n.dylib"
done
mc -c "$D/twolevel/main.c" -o main.o
ml -e _main main.o -la -lb -rpath @loader_path -o main
ml -e _main main.o -la -lb -rpath @loader_path -flat_namespace -o main-flat
ml -e _main main.o -la -lb -o main-norpath
for f in main main-flat main-norpath; do
 echo "== $f"; llvm-nm -m "$f"; llvm-otool -L "$f"; llvm-otool -l "$f" | grep -A2 LC_RPATH || true
 llvm-objdump --macho --chained-fixups "$f"
done
h '5 chained fixups'
mc -c "$D/fixups/ptrs.c" -o ptrs.o
ml -e _main ptrs.o -fixup_chains -o ptrs
ml -e _main ptrs.o -no_fixup_chains -o ptrs-old
mc -c "$D/fixups/ptrs2.c" -o ptrs2.o
ml -e _main ptrs2.o -fixup_chains -o ptrs2
for f in ptrs ptrs2; do
 echo "== $f"; llvm-nm -m "$f"; llvm-objdump --macho --chained-fixups "$f"; llvm-objdump -s -j __data "$f"
done
llvm-objdump --macho --rebase --bind ptrs-old
llvm-objdump --macho -d --section=__TEXT,__stubs prog
h '5.1 x86-64 lazy binding format'
clang --target=x86_64-apple-macos14 -O1 -c "$D/open/add.c" -o add-x86macho.o
ld64.lld -arch x86_64 -platform_version macos 14.0 14.0 -L. -lSystem -e _main \
 main-x86macho.o add-x86macho.o -no_fixup_chains -o prog-lazy
llvm-objdump --macho --lazy-bind prog-lazy
llvm-objdump -s -j __la_symbol_ptr prog-lazy
llvm-objdump -d prog-lazy
h '6 code signature bytes'
ml -e _main main-macho.o add-macho.o -o prog-nosig -no_adhoc_codesign
python3 "$D/sign/check-signature.py" prog
python3 "$D/sign/check-signature.py" prog-nosig
python3 "$D/sign/check-signature.py" prog --mutate bad
if python3 "$D/sign/check-signature.py" bad; then echo 'corruption unexpectedly accepted'; exit 1; fi
h '7 unwind records'
llvm-objdump --macho -s --section=__LD,__compact_unwind main-macho.o
llvm-objdump --macho --unwind-info prog
mc -c "$D/unwind/odd.s" -o odd.o
clang++ --target=arm64-apple-macos14 -O1 -c "$D/unwind/throw.cpp" -o throw.o
ml -e _main -undefined dynamic_lookup throw.o odd.o -o throw
llvm-objdump -h odd.o throw.o throw
llvm-objdump --macho --unwind-info throw
llvm-dwarfdump --eh-frame throw
h '8 PE import and relocation'
llvm-dlltool -m i386:x86-64 -d "$D/pe/kernel32.def" -l kernel32.lib
llvm-nm kernel32.lib
clang --target=x86_64-pc-windows-msvc -O1 -c "$D/pe/hello.c" -o hello.obj
llvm-objdump -dr hello.obj
lld-link /NODEFAULTLIB /ENTRY:main /SUBSYSTEM:CONSOLE /OUT:hello.exe hello.obj kernel32.lib
file hello.exe
llvm-readobj --file-headers --sections --coff-imports --coff-basereloc --unwind hello.exe
xxd -l 0x90 hello.exe
llvm-objdump -d -s hello.exe
python3 "$D/check-layout.py" main-macho.o hello.exe odd.o
if lld-link /NODEFAULTLIB /ENTRY:main /SUBSYSTEM:CONSOLE /OUT:x.exe hello.obj; then exit 1; fi
h '9 COMDAT'
clang++ --target=x86_64-pc-windows-msvc -O0 -c "$D/comdat/a.cpp" -o a.obj
llvm-readobj --sections --symbols a.obj
clang++ --target=arm64-apple-macos14 -O0 -c "$D/comdat/a.cpp" -o ca.o
clang++ --target=arm64-apple-macos14 -O0 -c "$D/comdat/b.cpp" -o cb.o
mc -c "$D/comdat/main.cpp" -o cmain.o
ml -e _main ca.o cb.o cmain.o -o cprog
llvm-nm -m ca.o; llvm-nm -m cprog
llvm-otool -hv cprog
llvm-objdump --macho --chained-fixups cprog
for f in exact1 exact2 anyc1 anyc2; do llvm-mc -triple x86_64-pc-windows-msvc -filetype=obj "$D/comdat/$f.s" -o "$f.obj"; done
clang --target=x86_64-pc-windows-msvc -O1 -c "$D/comdat/use.c" -o use.obj
lld-link /NODEFAULTLIB /ENTRY:main /SUBSYSTEM:CONSOLE /OUT:any.exe use.obj anyc1.obj anyc2.obj exact1.obj exact1.obj
llvm-objdump -s -j .rdata any.exe
if lld-link /NODEFAULTLIB /ENTRY:main /SUBSYSTEM:CONSOLE /OUT:ex.exe use.obj anyc1.obj exact1.obj exact2.obj; then exit 1; fi
h '10 TLS and WebAssembly'
mc -c "$D/tls/tls.c" -o tls.o
clang --target=x86_64-pc-windows-msvc -O1 -c "$D/tls/tls.c" -o tls.obj
for f in tls.o tls.obj; do llvm-objdump -h -dr "$f"; done
clang --target=wasm32 -O1 -c "$D/open/main.c" -o main.wasm.o
llvm-readobj --sections main.wasm.o
h '11 parser oracle and malformed input'
bash "$D/machoinfo/check-linux.sh"
echo 'Linux format experiments passed; only prog-elf was executed'
