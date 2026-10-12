#!/bin/sh
# 用 clang + ld.lld 在 Linux x86-64 主机上按几种调试选项各编一遍 xv6 内核。
# 用法：sh build.sh   产物在 $W/xv6/build-<变体>/kernel
set -e
W=${W:-$HOME/.cache/linker-lab/ch11}
SRC=$W/xv6
if [ ! -d "$SRC" ]; then
    git clone -q https://github.com/mit-pdos/xv6-riscv "$SRC"
    git -C "$SRC" checkout -q 06aad25
fi
cd "$SRC"
OBJS="entry start console printk uart kalloc spinlock string main vm proc swtch trampoline
      trap syscall sysproc bio fs log sleeplock file pipe exec sysfile kernelvec plic virtio_disk"
BASE="--target=riscv64-unknown-elf -march=rv64gc -mabi=lp64d -mcmodel=medany -O
      -fno-omit-frame-pointer -ffreestanding -fno-common -nostdlib -fno-builtin
      -fno-stack-protector -fno-pie -mno-relax -I. -w -fdebug-prefix-map=$SRC=/w"
build() {  # build 变体名 "编译选项" "链接选项"
    out=build-$1; mkdir -p $out && find $out -mindepth 1 -delete
    for o in $OBJS; do
        s=kernel/$o.c; [ -f $s ] || s=kernel/$o.S
        (cd $out && clang $BASE -I.. $2 -c ../$s -o $o.o)
    done
    (cd $out && ld.lld -z max-page-size=4096 -T ../kernel/kernel.ld $3 -o kernel $(for o in $OBJS; do printf '%s.o ' $o; done))
}
build g0    "-g0"
build dw4   "-gdwarf-4"
build dw5   "-gdwarf-5"
# 压缩变体：拿 build-dw5 的同一批目标文件重新链接，只差一个链接选项
LIST=$(for o in $OBJS; do printf 'build-dw5/%s.o ' $o; done)
ld.lld -z max-page-size=4096 -T kernel/kernel.ld --compress-debug-sections=zlib -o kernel-zlib $LIST
ld.lld -z max-page-size=4096 -T kernel/kernel.ld --compress-debug-sections=zstd -o kernel-zstd $LIST
llvm-objcopy --decompress-debug-sections kernel-zlib kernel-unz
# split DWARF 的 .dwo 打包成 .dwp
build split "-gdwarf-5 -gsplit-dwarf"
(cd build-split && llvm-dwp -o kernel.dwp *.dwo)
