#!/bin/sh
# Linux x86-64 原生实验；依赖 musl-tools、clang、lld、binutils、strace、python3。
# 每次创建独立目录，所有编译、链接、运行均发生在本机。
set -eu
[ "$(uname -sm)" = "Linux x86_64" ] || { echo 'requires native x86-64 Linux' >&2; exit 1; }
ulimit -c 0
D=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
W=$(mktemp -d "${TMPDIR:-/tmp}/linker-ch06.XXXXXX")
cp "$D"/*.c "$D"/start.s "$D"/patch.py "$W/"
cd "$W"
echo "WORKDIR=$W"
run() { printf '\n$ %s\n' "$*"; sh -c "$*"; }
for f in hello bsstail paging stack ctor tiny maps ex; do
  run "musl-gcc -static -no-pie -O1 $f.c -o $f"
done
run 'readelf -lW hello'
run 'objdump -d --no-show-raw-insn hello | sed -n "/<_start>:/,+21p"; nm -n hello | grep -E " (main|_init|_fini)$"' 
run 'readelf -lW bsstail; ./bsstail'
run './paging'
run 'musl-gcc -static-pie -O1 spie_maps.c -o spie_maps; ./spie_maps; ./spie_maps'
run 'musl-gcc -pie -fPIE -O1 spie_maps.c -o pie_dyn; ./pie_dyn; ./pie_dyn'
run 'env -i HOME=/root PATH=/bin ./stack hi'
run 'clang -O1 -c sse.c; clang -x assembler-with-cpp -c start.s -o start_good.o; ld.lld -static start_good.o sse.o -o sse_good'
run 'clang -DMISALIGN -x assembler-with-cpp -c start.s -o start_bad.o; ld.lld -static start_bad.o sse.o -o sse_bad'
run './sse_good; echo good=$?; ./sse_bad; echo bad=$?'
run 'objdump -s -j .init_array -j .fini_array ctor; nm -n ctor | grep -E " (before|after|frame_dummy|__do_global_dtors_aux)$"; ./ctor'
run 'musl-gcc -v -static -no-pie -O1 hello.c -o hello'
run 'musl-gcc -O1 -c hello.c -o hello.o; S=/usr/lib/x86_64-linux-musl; G=$(dirname "$(gcc -print-libgcc-file-name)"); ld -static -o hello_manual $S/Scrt1.o $S/crti.o $G/crtbeginS.o hello.o -L$S -lc $G/crtendS.o $S/crtn.o; ./hello; ./hello_manual; cmp hello hello_manual || true'
run 'musl-gcc -static-pie -fPIE -O1 spie.c -o spie; readelf -hlW spie; readelf -rW spie; nm spie | grep -wE "ops|add|sub|greeting"; ./spie; ./spie'
run 'musl-gcc -O1 hello.c -o hello_dyn; readelf -lW hello_dyn'
run 'readelf -lW tiny; readelf -lW maps; ./maps; readelf -lW ex; ./ex'
python3 - <<'PATCH'
import struct,subprocess
b=open('tiny','rb').read(); off=struct.unpack_from('<Q',b,32)[0]; n=struct.unpack_from('<H',b,56)[0]
ph=[struct.unpack_from('<IIQQQQQQ',b,off+56*i) for i in range(n)]
rw=next(i for i,p in enumerate(ph) if p[0]==1 and p[1]&2)
rx=next(i for i,p in enumerate(ph) if p[0]==1 and p[1]&1)
for name,field,value,idx in [('bad_machine','e_machine',0x1234,None),('bad_entry','e_entry',0xdead0000,None),('bad_filesz','p_filesz',ph[rw][6]+1,rw),('bad_memsz','p_memsz',0xfffffffffff00000,rw),('bad_wrap','p_memsz',(0x1000-ph[rw][3])%(1<<64),rw),('bad_offset','p_offset',ph[rw][2]+0x100000,rw),('bad_offset_text','p_offset',ph[rx][2]+0x100000,rx),('e1','e_entry',0x400000,None),('e3','p_filesz',0x28,rw)]:
 cmd=['python3','patch.py','tiny',name,field,hex(value)]+([] if idx is None else [str(idx)])
 print('$ '+' '.join(cmd),flush=True); subprocess.run(cmd,check=True)
b=open('maps','rb').read(); off=struct.unpack_from('<Q',b,32)[0]; n=struct.unpack_from('<H',b,56)[0]
i=next(i for i in range(n) if struct.unpack_from('<I',b,off+56*i)[0]==0x6474e551)
subprocess.run(['python3','patch.py','maps','e2','p_flags','7',str(i)],check=True)
PATCH
for f in bad_machine bad_entry bad_filesz bad_memsz bad_wrap bad_offset bad_offset_text e1 e3; do
  run "strace -e trace=execve ./$f; echo status=\$?"
done
run 'readelf -lW e2 | grep GNU_STACK; ./e2 | grep stack; ./maps | grep stack'
sh "$D/overlap.sh" "$W"
echo 'completed Linux native ch06'
