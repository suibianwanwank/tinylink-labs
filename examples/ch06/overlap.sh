#!/bin/sh
set -eu
ulimit -c 0
cd "$1"
cat > overlap.s <<'EOF'
.text
.globl _start
_start:
 mov value(%rip), %edi
 mov $60, %eax
 syscall
#ifdef LONG
 .skip 4096
#endif
.data
value: .long 42
.section .note.GNU-stack,"",@progbits
EOF
cat > overlap.ld <<'EOF'
ENTRY(_start)
PHDRS { code PT_LOAD FLAGS(5); data PT_LOAD FLAGS(6); }
SECTIONS { .text 0x201000 : { *(.text) } :code .data 0x201800 : { *(.data) } :data }
EOF
clang -x assembler-with-cpp -c overlap.s -o shared.o
clang -DLONG -x assembler-with-cpp -c overlap.s -o overlap.o
ld.lld --no-check-sections -T overlap.ld shared.o -o shared
ld.lld --no-check-sections -T overlap.ld overlap.o -o overlap
readelf -lW shared
readelf -lW overlap
set +e
strace -e trace=execve ./shared
strace -e trace=execve ./overlap
S=/usr/lib/x86_64-linux-musl
G=$(dirname "$(gcc -print-libgcc-file-name)")
ld -static -o nocrt1 "$S/crti.o" "$G/crtbeginS.o" hello.o -L"$S" -lc "$G/crtendS.o" "$S/crtn.o"
strace -e trace=execve ./nocrt1
nm -n hello | grep -E ' main$|_start$|__init_ssp$'
