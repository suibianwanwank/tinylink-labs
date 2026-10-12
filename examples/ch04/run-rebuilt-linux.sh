#!/usr/bin/env bash
set -euo pipefail
[[ $(uname -s) == Linux && $(uname -m) == x86_64 ]] || { echo 'Requires native x86-64 Linux' >&2; exit 1; }
for t in as ld objdump readelf objcopy python3; do command -v "$t" >/dev/null; done
work=$(mktemp -d /tmp/theory04-rebuild.XXXXXX)
cd "$work"
echo "WORK=$work"
as --version | head -n 1
ld --version | head -n 1
cat > start.s <<'EOF'
.section .text.start,"ax",@progbits
.global _start
.type _start,@function
_start:
    mov $40, %edi
    mov $2, %esi
    call add
    mov %eax, %edi
    mov $60, %eax
    syscall
.size _start, .-_start
.section .note.GNU-stack,"",@progbits
EOF
cat > add.s <<'EOF'
.section .text.add,"ax",@progbits
.global add
.type add,@function
add:
    lea (%rdi,%rsi), %eax
    ret
.size add, .-add
.section .note.GNU-stack,"",@progbits
EOF
cat > fixed.ld <<'EOF'
ENTRY(_start)
SECTIONS {
  .text 0x401000 : { *(.text.start) . = ALIGN(32); *(.text.add) }
  .data 0x402000 : { *(.data) }
}
EOF
as --64 start.s -o start.o
as --64 add.s -o add.o
ld -T fixed.ld start.o add.o -o app
status=0; ./app || status=$?; test "$status" -eq 42
objcopy -O binary --only-section=.text app text.bin
python3 - <<'PY'
from pathlib import Path
b=Path('text.bin').read_bytes()
assert b[10:15] == bytes.fromhex('e8 11 00 00 00'),b.hex()
print('forward call e8 11 00 00 00; status 42')
PY
readelf -rW start.o
objdump -d app
cat > backward.ld <<'EOF'
ENTRY(_start)
SECTIONS {
  .text 0x401000 : { *(.text.add) . = ALIGN(32); *(.text.start) }
  .data 0x402000 : { *(.data) }
}
EOF
ld -T backward.ld start.o add.o -o backward
status=0; ./backward || status=$?; test "$status" -eq 42
objcopy -O binary --only-section=.text backward backward.bin
python3 - <<'PY'
from pathlib import Path
b=Path('backward.bin').read_bytes()
assert b[0x2a:0x2f] == bytes.fromhex('e8 d1 ff ff ff'),b.hex()
print('backward call e8 d1 ff ff ff; status 42')
PY
cat > data.s <<'EOF'
.section .data,"aw",@progbits
.global slot, counter, flag
slot: .quad counter + 4
counter: .long 40, 2
flag: .byte 0
.section .note.GNU-stack,"",@progbits
EOF
as --64 data.s -o data.o
ld -T fixed.ld start.o add.o data.o -o pointers
objcopy -O binary --only-section=.data pointers data.bin
python3 - <<'PY'
from pathlib import Path
b=Path('data.bin').read_bytes()
assert b[:8] == bytes.fromhex('0c 20 40 00 00 00 00 00'),b.hex()
print('absolute pointer 0c 20 40 00 00 00 00 00')
PY
cat > addends.s <<'EOF'
.text
.extern counter, flag
    cmpl $1001, counter(%rip)
    movb $1, flag(%rip)
    ret
.section .note.GNU-stack,"",@progbits
EOF
as --64 addends.s -o addends.o
readelf -rW addends.o > addends.txt
cat addends.txt
python3 - <<'PY'
from pathlib import Path
s=Path('addends.txt').read_text()
assert 'counter - 8' in s and 'flag - 5' in s,s
PY
cat > widths.s <<'EOF'
.section .text.start,"ax",@progbits
.global _start
_start:
    mov $target, %eax
    mov $target, %rax
    movabs $target, %rax
    mov $60, %eax
    xor %edi, %edi
    syscall
.section .note.GNU-stack,"",@progbits
EOF
as --64 widths.s -o widths.o
readelf -rW widths.o
if ld -T fixed.ld --defsym=target=0x80000000 widths.o -o overflow > overflow.log 2>&1; then echo 'expected 32S rejection' >&2; exit 1; fi
cat overflow.log
ld -T fixed.ld --defsym=target=0x7fffffff widths.o -o widths-ok
cat > got.s <<'EOF'
.text
.global get
.hidden counter
get:
    mov counter@GOTPCREL(%rip), %rax
    mov (%rax), %eax
    ret
.data
.global counter
counter: .long 40
.section .note.GNU-stack,"",@progbits
EOF
as --64 got.s -o got.o
ld -pie -e get got.o -o got-relax
ld -pie --no-relax -e get got.o -o got-no-relax
objdump -d got-relax > got-relax.txt
objdump -d got-no-relax > got-no-relax.txt
readelf -rW got.o
cat got-relax.txt got-no-relax.txt
python3 - <<'PY'
from pathlib import Path
assert 'lea' in Path('got-relax.txt').read_text()
assert 'mov' in Path('got-no-relax.txt').read_text()
PY
if command -v ld.lld >/dev/null; then
 ld.lld -T fixed.ld start.o add.o -o app-lld
 status=0; ./app-lld || status=$?; test "$status" -eq 42
 objcopy -O binary --only-section=.text app-lld lld.bin
 python3 - <<'PY'
from pathlib import Path
assert Path('lld.bin').read_bytes()[10:15] == bytes.fromhex('e8 11 00 00 00')
PY
fi
echo 'PASS: forward, backward, pointer, -8/-5 addends, 32S boundary, GOT relaxation'

sed 's/1001, counter(%rip)/1001, counter+4(%rip)/' addends.s > addends-plus.s
as --64 addends-plus.s -o addends-plus.o
readelf -rW addends-plus.o
sed '/mov \$target, %rax/d' widths.s > widths32.s
as --64 widths32.s -o widths32.o
ld -T fixed.ld --defsym=target=0x80000000 widths32.o -o widths32
if ld -T fixed.ld --defsym=target=0x100000000 widths32.o -o widths32-bad > overflow32.log 2>&1; then exit 1; fi
cat overflow32.log

if command -v clang >/dev/null && command -v ld.lld >/dev/null && command -v llvm-objdump >/dev/null; then
cat > a64.s <<'EOF'
.text
.global probe
probe:
 adrp x0, target
 add x0, x0, :lo12:target
 ret
EOF
cat > a64.ld <<'EOF'
ENTRY(probe)
SECTIONS { .text 0x400100 : { *(.text) } target = 0x412678; }
EOF
clang --target=aarch64-linux-gnu -c a64.s -o a64.o
ld.lld --no-relax -T a64.ld a64.o -o a64
llvm-objdump -d a64
echo PASS-extras

else
 echo "SKIP: optional AArch64 encoding comparison needs Clang/LLD/llvm-objdump"
fi
