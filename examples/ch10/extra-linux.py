#!/usr/bin/env python3
"""Chapter 10 exercises; W is the native run-all.sh artifact directory."""
from pathlib import Path
import subprocess,tempfile,shutil,os
base=Path(os.environ['W']); src=base/'xv6';work=Path(tempfile.mkdtemp(prefix='linker-ch10-ex-'))
print('work',work,flush=True)
def run(cmd,cwd=src,timeout=40,okay=(0,)):
 p=subprocess.run(cmd,shell=True,cwd=cwd,text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=timeout);print('$',cmd,'\n'+p.stdout,flush=True)
 assert p.returncode in okay,(cmd,p.returncode)
 return p.stdout
names='start console printk uart kalloc spinlock string main vm proc swtch trampoline trap syscall sysproc bio fs log sleeplock file pipe exec sysfile kernelvec plic virtio_disk entry'.split();objs=' '.join('kernel/'+n+'.o' for n in names)
ld='riscv64-unknown-elf-ld -m elf64lriscv -z max-page-size=4096'
script=str(src/'kernel/kernel.ld');fixed=work/'fixed.ld';fixed.write_text(Path(script).read_text().replace('kernel/entry.o(_entry)','kernel/entry.o(.text)'))
for n,c in [('k2',f'{ld} -T {script} -o {work}/k2 {objs}'),('k3',f'{ld} -o {work}/k3 {objs} -T {script}'),('k5',f'ld.lld -z max-page-size=4096 -T {script} -o {work}/k5 {objs}'),('k6',f'ld.lld -z max-page-size=4096 -T {fixed} -o {work}/k6 {objs}')]:
 run(c);run(f"riscv64-unknown-elf-nm -n {work}/{n} | grep -E ' (_entry|timerinit)$'")
# Boot original with QEMU CPU records; then inspect the exact start PCs.
run(f'timeout --kill-after=1 3 qemu-system-riscv64 -machine virt -bios none -kernel kernel/kernel -m 128M -smp 3 -nographic -d cpu,nochain -D {work}/cpu.log >/dev/null 2>&1',okay=(124,))
s=(work/'cpu.log').read_text();seen=set()
import re
for b in s.split(' V      ='):
 if 'pc       0000000080000058' in b:
  hart=re.search(r'mhartid  (\w+)',b);sp=re.search(r'x2/sp\s+(\w+)',b)
  if hart and sp and hart.group(1) not in seen:print('START',hart.group(1),sp.group(1));seen.add(hart.group(1))
(work/'cpu.log').unlink()
# Instrument a copy, preserving the baseline objects.
copy=work/'xv6';shutil.copytree(src,copy,ignore=shutil.ignore_patterns('.git'))
p=copy/'kernel/kalloc.c';s=p.read_text().replace('  freerange(end, (void *)PHYSTOP);','  freerange(end, (void *)PHYSTOP);\n  int pages=0; for(struct run *r=kmem.freelist;r;r=r->next) pages++;\n  printk("kinit: end=%p first=%p pages=%d\\n", end, (void*)PGROUNDUP((uint64)end), pages);');p.write_text(s)
run('make -s TOOLPREFIX=riscv64-unknown-elf- CC="riscv64-unknown-elf-gcc -mabi=lp64d" LD="'+ld+'" kernel/kernel',cwd=copy)
run('timeout --kill-after=1 4 qemu-system-riscv64 -machine virt -bios none -kernel kernel/kernel -m 128M -smp 3 -nographic',cwd=copy,okay=(124,))
# Only static negative link checks; never alter the baseline trampoline.
p=work/'tramp_big.S';p.write_text((src/'kernel/trampoline.S').read_text()+'\n.section trampsec\n.space 4096\n');run(f'riscv64-unknown-elf-gcc -mabi=lp64d -march=rv64gc -I. -Ikernel -c {p} -o {work}/tramp_big.o')
for name,cmd in [('GNU',ld),('LLD','ld.lld -z max-page-size=4096')]:
 run(f'{cmd} -T {script} -o {work}/bad {objs.replace("kernel/trampoline.o",str(work/"tramp_big.o"))}',okay=(1,))
 run(f'{cmd} -T {script} -o {work}/bad {objs.replace("kernel/trampoline.o","")}',okay=(1,))
print('PASS ch10 extra exercises')
