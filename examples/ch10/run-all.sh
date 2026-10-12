#!/bin/sh
# 在 Linux x86-64 主机编译、链接；RISC-V 程序由 QEMU 整机运行。
# 依赖 Clang/LLD/LLVM、riscv64-unknown-elf-{gcc,binutils}、QEMU、make、Rust。
set -e
D=$(cd "$(dirname "$0")" && pwd)
[ "$(uname -s)" = Linux ] || { echo "requires Linux" >&2; exit 2; }
if [ -n "${W:-}" ]; then mkdir "$W"; else W=$(mktemp -d "${TMPDIR:-/tmp}/linker-ch10.XXXXXX"); fi
export W
printf "experiment directory: %s\n" "$W"
CF="--target=riscv64-unknown-elf -march=rv64gc -mabi=lp64d -mno-relax -ffreestanding -fno-builtin -nostdlib -O1"
CC="clang $CF -mcmodel=medany"

for d in bare embed ex misc lseval; do
    mkdir "$W/$d" && cp -R "$D/$d/." "$W/$d/"
done
ok()   { echo "ok: $*"; }
fail() { echo "FAIL: $*" >&2; exit 1; }
has()  { grep -qE -- "$2" "$1" || { cat "$1" >&2; fail "$1 里没有 /$2/"; }; ok "$1 ~ /$2/"; }
hasnt(){ ! grep -qE -- "$2" "$1" || { cat "$1" >&2; fail "$1 里不该有 /$2/"; }; ok "$1 !~ /$2/"; }
q()    { bash "$W/bare/run.sh" "$@"; }   # 跑 2 秒后结束 QEMU
eq()   { [ "$1" = "$2" ] || fail "$3：期望 $2，得到 $1"; ok "$3 = $2"; }
size() { wc -c <"$1" | tr -d ' '; }
after_text() { grep -A1 '\] \.text ' "$1" | grep -q '\.mytable' || fail "$1：.mytable 不在 .text 之后"; ok "$1：.mytable 紧跟 .text"; }
bad()  { out=$1; shift; if "$@" >"$out" 2>&1; then fail "$* 应当失败"; fi; }

echo "== 1. 主机：编译与 ld.lld"
cd "$W/bare"
bash build.sh link.ld hello.elf
q hello.elf >hello.out;                         has hello.out '^4: done'
ld.lld -T link_bad.ld -o bad.elf entry.o main.o && llvm-objcopy -O binary bad.elf bad.bin
q bad.elf >bad_elf.out;                         hasnt bad_elf.out 'via'
q bad.bin -d int -D bad_bin.log >bad_bin.out;   has bad_bin.out '^1: via pc-relative literal'
hasnt bad_bin.out '^2:';                        has bad_bin.log 'epc:0x0*80200016.*illegal_instruction'
clang $CF -mcmodel=medlow -c main.c -o main_medlow.o
bad medlow.out ld.lld -T link.ld -o medlow.elf entry.o main_medlow.o
has medlow.out 'R_RISCV_HI20 out of range'
ld.lld -T high.ld -o high.elf entry.o main.o && llvm-readelf -lW high.elf >high.txt
has high.txt 'LOAD +0x001000 0xffffffff80000000 0x0000000080000000 0x0000e4'
q high.elf >high.out;                           has high.out '^1: via'; hasnt high.out '^2:'
$CC -c entry_rom.S -o entry_rom.o && $CC -c entry_nocopy.S -o entry_nocopy.o
ld.lld -T rom.ld -o rom.elf entry_rom.o main.o && llvm-nm -n rom.elf >rom.nm
has rom.nm '0000000080000188 A _sidata'
q rom.elf >rom.out;                             has rom.out '^4: done'
ld.lld -T rom.ld -o nocopy.elf entry_nocopy.o main.o
q nocopy.elf -d int -D nc.log >nc.out;          has nc.log 'desc=fault_load'
ld.lld -T rom_noat.ld -o rom_noat.elf entry_rom.o main.o && llvm-nm rom_noat.elf >noat.nm
has noat.nm '0000000080100000 A _sidata'
llvm-objcopy -O binary rom_noat.elf noat.bin
eq "$(size noat.bin)" 1048592 noat.bin
q rom_noat.elf >noat.out;                       has noat.out '^4: done'

cd "$W/embed"
cp ../bare/link.ld ../bare/entry.o .
$CC -c show.c -o show.o && $CC -c incbin.S -o incbin.o && $CC -std=c23 -c embed.c -o embed.o
llvm-readelf -hsW incbin.o embed.o | grep -E 'Flags:|blob' >flags.txt
has flags.txt '12 OBJECT +GLOBAL DEFAULT +3 blob$'; hasnt flags.txt 'Flags: +0x0,'
ld.lld -T link.ld -o e_lld.elf entry.o show.o -b binary data/msg-v1.txt -b default
q e_lld.elf >e_lld.out;                         has e_lld.out '^hello, blob'; has e_lld.out '^12$'
rustc --crate-type=lib --emit=obj -O blob.rs -o blob_rs.o
llvm-readelf -SsW blob_rs.o >blob_rs.txt;       has blob_rs.txt '\.data\.rel\.ro\.BLOB'

cd "$W/misc"
$CC -ffunction-sections -c calls.c -o calls.o && $CC -c disc.c -o disc.o && $CC -c ins.c -o ins.o
ld.lld --gc-sections -T calls.ld -o c_lld.elf calls.o && llvm-nm -n c_lld.elf >c_lld.nm
has c_lld.nm '0000000080000038 d __initcall_b'; has c_lld.nm '0000000080000048 d __initcall_a'
has c_lld.nm '0000000080000050 . marker'
ld.lld --gc-sections -T nosort.ld -o ns calls.o && llvm-nm -n ns >ns.nm
has ns.nm '0000000080000038 d __initcall_a'
bad nokeep_lld.out ld.lld --gc-sections -T nokeep.ld -o x calls.o; has nokeep_lld.out 'expected 3 initcalls'
bad disc_lld.out ld.lld -T disc.ld -o d disc.o; has disc_lld.out 'discarded section: setup'
ld.lld -T ins.ld -o i_lld ins.o 2>/dev/null && llvm-readelf -SW i_lld >i_lld.txt
after_text i_lld.txt

cd "$W/ex"
$CC -c sizes.S -o sizes.o
ld.lld -T fw.ld -o fw_lld.elf sizes.o && llvm-objcopy -O binary fw_lld.elf fw_lld.bin
eq "$(size fw_lld.bin)" 584 fw_lld.bin

echo "== 2. Linux：GNU ld、xv6、secs.sh"
X=$W/xv6
if [ ! -d "$X/.git" ]; then
    src=https://github.com/mit-pdos/xv6-riscv
    [ -d "$HOME/.cache/linker-lab/ch10/xv6/.git" ] && src=$HOME/.cache/linker-lab/ch10/xv6
    git clone -q "$src" "$X"
fi
git -C "$X" checkout -q 06aad25
(
LD="riscv64-unknown-elf-ld -m elf64lriscv"; RE=riscv64-unknown-elf-readelf
cd "$W"/bare
$LD -T high.ld -o high_bfd.elf entry.o main.o 2>high_bfd.log; $RE -lW high_bfd.elf >high_bfd.txt
$LD -T rom.ld -o rom_bfd.elf entry_rom.o main.o; riscv64-unknown-elf-objdump -h rom_bfd.elf >rom_bfd.txt
cd "$W"/embed
$LD -r -b binary -o blob_bfd.o data/msg-v1.txt; $RE -SsW blob_bfd.o >blob_bfd.txt
riscv64-unknown-elf-objcopy -I binary -O elf64-littleriscv -B riscv \
    --rename-section .data=.rodata.blob,alloc,load,readonly,data,contents data/msg-v1.txt blob_objcopy.o
$LD -T link.ld -o e_bfd.elf entry.o show.o -b binary data/msg-v1.txt -b elf64-littleriscv
cd "$W"/misc
$LD --gc-sections -T calls.ld -o c_bfd.elf calls.o; riscv64-unknown-elf-nm -n c_bfd.elf >c_bfd.nm
$RE -SW c_bfd.elf >c_bfd.txt
$LD --gc-sections -T nokeep.ld -o x calls.o >nokeep_bfd.out 2>&1 || true
$LD -T disc.ld -o d disc.o >disc_bfd.out 2>&1 || true
$LD -T ins.ld -o i_bfd ins.o 2>/dev/null; $RE -SW i_bfd >i_bfd.txt
cd "$W"/ex
$LD -T fw.ld -o fw_bfd.elf sizes.o; riscv64-unknown-elf-objcopy -O binary fw_bfd.elf fw_bfd.bin
$LD -T fw_al.ld -o fw_al.elf sizes.o; riscv64-unknown-elf-objcopy -O binary fw_al.elf fw_al.bin
cd "$W"/xv6
make -s TOOLPREFIX=riscv64-unknown-elf- CC="riscv64-unknown-elf-gcc -mabi=lp64d" LD="$LD" kernel/kernel fs.img >make.log 2>&1
U="user/cat.o user/ulib.o user/usys.o user/printf.o user/umalloc.o"
K=""; for o in entry start console printk uart kalloc spinlock string main vm proc swtch trampoline \
    trap syscall sysproc bio fs log sleeplock file pipe exec sysfile kernelvec plic virtio_disk; do K="$K kernel/$o.o"; done
sh "$W"/lseval/secs.sh $U >"$W"/cat.secs; sh "$W"/lseval/secs.sh $K >"$W"/kernel.secs
$LD -z max-page-size=4096 --no-relax -T user/user.ld -o cat_norelax $U; $RE -SW cat_norelax >cat_norelax.txt
$LD -z max-page-size=4096 --no-relax -T kernel/kernel.ld -o k_nr $K; riscv64-unknown-elf-nm -n k_nr >k_nr.nm
)
cd "$W/bare"; has high_bfd.log 'LOAD segment with RWX'
has high_bfd.txt 'LOAD +0x001000 0xffffffff80000000 0x0000000080000000 0x000150'
has rom_bfd.txt '\.bss +00000000 +0000000080100010 +0000000080000198'
q rom_bfd.elf >rom_bfd.out;                     has rom_bfd.out '^4: done'
cd "$W/embed"; has blob_bfd.txt '000000000000000c +0 NOTYPE +GLOBAL DEFAULT +ABS _binary_data_msg_v1_txt_size'
bad x_lld.out ld.lld -T link.ld -o x.elf entry.o show.o blob_bfd.o; has x_lld.out 'different floating-point ABI'
bad x2_lld.out ld.lld -T link.ld -o x.elf entry.o show.o blob_objcopy.o; has x2_lld.out 'different floating-point ABI'
q e_bfd.elf >e_bfd.out;                         has e_bfd.out '^hello, blob'; has e_bfd.out '^12$'
cd "$W/misc"; has c_bfd.nm '0000000080000038 d __initcall_b'; has c_bfd.nm '0000000080000050 . marker'
hasnt c_bfd.txt '\.empty'; has nokeep_bfd.out 'expected 3 initcalls'; has disc_bfd.out 'defined in discarded section'
after_text i_bfd.txt
cd "$W/ex"
eq "$(size fw_bfd.bin)" 577 fw_bfd.bin
eq "$(size fw_al.bin)" 584 "fw_al.bin（ALIGN(8)）"

echo "== 3. 主机：xv6 启动与 lseval"
cd "$X"
(sleep 4; printf 'echo hello xv6\n'; sleep 2) | qemu-system-riscv64 -machine virt -bios none -kernel kernel/kernel \
    -m 128M -smp 3 -nographic -global virtio-mmio.force-legacy=false \
    -drive file=fs.img,if=none,format=raw,id=x0 -device virtio-blk-device,drive=x0,bus=virtio-mmio-bus.0 >boot.out 2>&1 &
pid=$!; sleep 7; kill $pid 2>/dev/null || true; wait $pid 2>/dev/null || true
has boot.out 'init: starting sh'; has boot.out '^hello xv6'
has cat_norelax.txt '\] \.text +PROGBITS +0000000000000000 001000 000a46'
has k_nr.nm '0000000080021bb0 B end'
cd "$W"
rustc -O --edition 2024 lseval/lseval.rs -o lseval/lseval
grep -v trampsec kernel.secs >no-trampsec.secs
printf 'SECTIONS {\n  .text : { *(.text) }\n  . = ;\n}\n' >bad.ld
eq "$(grep -c . cat.secs)" 78 "cat.secs 行数"
lseval/lseval xv6/user/user.ld cat.secs >lseval_cat.out;       has lseval_cat.out '^\.text +0x0 大小 0xa46'
lseval/lseval xv6/kernel/kernel.ld kernel.secs >lseval_k.out;  has lseval_k.out '^符号 end = 0x80021bb0'
has lseval_k.out '^符号 _trampoline = 0x80007000'
lseval/lseval xv6/kernel/kernel.ld no-trampsec.secs >lseval_nt.out 2>&1 || true
has lseval_nt.out 'kernel.ld:19:5: ASSERT 失败'
lseval/lseval bad.ld cat.secs >lseval_bad.out 2>&1 || true;   has lseval_bad.out 'bad.ld:3:7:'
python3 "$D/extra-linux.py"
echo "== 第 10 章全部通过"
