#!/bin/sh
# 统计 build.sh 各变体的调试节大小、重定位条数、链接输入字节数
W=${W:-$HOME/.cache/linker-lab/ch11}
set -eu
cd "$W/xv6"
SCRATCH=$(mktemp -d "${TMPDIR:-/tmp}/linker-ch11-analyze.XXXXXX")
trap 'rm -rf "$SCRATCH"' EXIT HUP INT TERM
dbg() { llvm-size -A "$1" | awk '$1 ~ /^\.debug/ {s+=$2; n++} END {printf "%d 个 .debug_* 节，合计 %d 字节\n", n, s}'; }
for k in build-g0/kernel build-dw4/kernel build-dw5/kernel kernel-zlib kernel-zstd kernel-unz build-split/kernel; do
    v=$k
    printf '%-18s 文件 %7d 字节；' $v $(wc -c < $k); dbg $k
done
echo "--- 输出里各调试节（dw4 / dw5）"
for v in dw4 dw5; do echo "[$v]"; llvm-size -A build-$v/kernel | awk '$1 ~ /^\.debug/ {printf "  %-20s %6d\n", $1, $2}'; done
echo "--- 目标文件里的重定位条数（全部 27 个 .o 合计）"
for v in g0 dw4 dw5 split; do
    t=0; d=0
    for o in build-$v/*.o; do
        a=$(llvm-readelf -rW $o | awk '/^[0-9a-f]{16} / {n++} END {print n+0}')
        b=$(llvm-readelf -rW $o | awk '/^Relocation section/ {dbg = ($3 ~ /debug/)} /^[0-9a-f]{16} / && dbg {n++} END {print n+0}')
        t=$((t+a)); d=$((d+b))
    done
    echo "$v 共 $t 条，其中 .rela.debug_* $d 条"
done
echo "--- 按节细分（dw4 / dw5）"
for v in dw4 dw5; do echo "[$v]"; for o in build-$v/*.o; do llvm-readelf -rW $o | awk '/^Relocation section/ {s=$3; gsub(/\x27/,"",s)} /^[0-9a-f]{16} / {print s}'; done | sort | uniq -c | sort -rn | grep debug; done
echo "--- 链接输入字节数"
for v in g0 dw5 split; do printf '%-6s .o 合计 %8d 字节\n' $v $(cat build-$v/*.o | wc -c); done
printf 'split  .dwo 合计 %8d 字节\n' $(cat build-split/*.dwo | wc -c)
printf 'split  .dwp      %8d 字节\n' $(wc -c < build-split/kernel.dwp)
echo "--- 压缩头（.debug_info 前 24 字节）"
for k in kernel-zlib kernel-zstd; do llvm-readelf -x .debug_info $k | sed -n 3,4p; done
echo "--- .debug_str 合并：输入合计 / 输出"
for o in build-dw5/*.o; do llvm-size -A $o | awk '$1==".debug_str"{print $2}'; done | awk '{s+=$1} END {print "输入合计", s}'
llvm-size -A build-dw5/kernel | awk '$1==".debug_str"{print "输出", $2}'
LIST=$(for o in entry start console printk uart kalloc spinlock string main vm proc swtch trampoline trap syscall sysproc bio fs log sleeplock file pipe exec sysfile kernelvec plic virtio_disk; do printf 'build-dw5/%s.o ' $o; done)
ld.lld -z max-page-size=4096 -T kernel/kernel.ld -O2 -o ${SCRATCH}/k-O2 $LIST
llvm-size -A ${SCRATCH}/k-O2 | awk '$1==".debug_str"{print "-O2 输出", $2}'
llvm-objcopy --dump-section .debug_str=${SCRATCH}/o2.bin ${SCRATCH}/k-O2 ${SCRATCH}/x3
python3 -c "d=open('${SCRATCH}/o2.bin','rb').read(); print('-O2 输出里有单独的 lock:', b'\\0lock\\0' in d or d.startswith(b'lock\\0'))"
llvm-dwarfdump --debug-info ${SCRATCH}/k-O2 | grep -m1 '("lock")'
rm -f ${SCRATCH}/o2.bin ${SCRATCH}/x3
echo "--- 链接时 -S"
ld.lld -z max-page-size=4096 -T kernel/kernel.ld -S -o ${SCRATCH}/k-S $LIST
printf '%d 字节，.debug 节 %d 个\n' $(wc -c < ${SCRATCH}/k-S) $(llvm-readelf -SW ${SCRATCH}/k-S | grep -c '\.debug')
rm -f ${SCRATCH}/k-O2 ${SCRATCH}/k-S
echo "--- 把 .dwo 挪走再链接一次 split 变体"
mkdir -p ${SCRATCH}/dwo-hide && mv build-split/*.dwo ${SCRATCH}/dwo-hide/
LIST=$(for o in entry start console printk uart kalloc spinlock string main vm proc swtch trampoline trap syscall sysproc bio fs log sleeplock file pipe exec sysfile kernelvec plic virtio_disk; do printf 'build-split/%s.o ' $o; done)
ld.lld -z max-page-size=4096 -T kernel/kernel.ld -o ${SCRATCH}/k-nodwo $LIST && cmp ${SCRATCH}/k-nodwo build-split/kernel && echo "与有 .dwo 时的产物逐字节相同"
mv ${SCRATCH}/dwo-hide/*.dwo build-split/ && rm -f ${SCRATCH}/k-nodwo
echo "--- split 变体输出里的调试节"
llvm-size -A build-split/kernel | grep debug
echo "--- kernel-zlib 的 .debug_info 节头"
llvm-readelf -SW kernel-zlib | grep -E ' \.debug_info '
echo "--- 解压回来的调试节与原来逐字节比较"
for s in .debug_info .debug_line .debug_str .debug_addr; do
    llvm-objcopy --dump-section $s=${SCRATCH}/a.bin build-dw5/kernel ${SCRATCH}/x1
    llvm-objcopy --dump-section $s=${SCRATCH}/b.bin kernel-unz ${SCRATCH}/x2
    cmp ${SCRATCH}/a.bin ${SCRATCH}/b.bin && echo "$s 相同"
done
rm -f ${SCRATCH}/x1 ${SCRATCH}/x2 ${SCRATCH}/a.bin ${SCRATCH}/b.bin
