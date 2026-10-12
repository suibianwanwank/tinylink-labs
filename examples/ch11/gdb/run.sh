#!/bin/sh
# 第 11 章"分离调试文件"和"split DWARF"两节的实验，在原生 x86-64 Linux 上运行。
# 需要 gcc、binutils 与 gdb。用法：sh run.sh
set -e
W=${W:-$HOME/.cache/linker-lab/ch11}/gdb
mkdir -p "$W" && cp "$(dirname "$0")"/main.c "$(dirname "$0")"/util.c "$W/"
cd "$W"
(
G="gdb -q -batch -iex \"set debug-file-directory $W/debug-root\" -ex \"break add\" -ex run -ex bt"
V="gdb -q -batch -iex \"set debug-file-directory $W/debug-root\" -iex \"set verbose on\" -ex \"info line add\""
set -x
for d in dbg sp; do mkdir -p $d && find $d -mindepth 1 -delete; done
gcc -O0 -g main.c util.c -o prog
readelf -n prog | grep -A1 "Build ID"
objcopy --only-keep-debug prog dbg/prog.debug
objcopy --strip-debug --strip-unneeded prog dbg/prog
objcopy --add-gnu-debuglink=dbg/prog.debug dbg/prog
readelf -x .gnu_debuglink dbg/prog
ls -l prog dbg/prog dbg/prog.debug
readelf -SW dbg/prog.debug | grep -E "\.text|\.debug_info|\.data "
cp dbg/prog.debug prog.debug.orig
cd dbg
eval $V ./prog 2>&1
eval $G ./prog 2>&1
# 换个名字放到别处：debuglink 找不到，改走 build-id
id=$(readelf -n prog | sed -n "s/.*Build ID: //p")
B=$W/debug-root/.build-id/$(echo $id | cut -c1-2)/$(echo $id | cut -c3-).debug
mkdir -p $(dirname $B)
mv prog.debug $B
ls -l $B
eval $V ./prog 2>&1
# 改坏：debuglink 指向的文件内容变了，CRC 对不上
mv $B prog.debug
printf x >> prog.debug
eval $V ./prog 2>&1 || true
rm prog.debug
eval $V ./prog 2>&1 || true
cd ..
# split DWARF
cd sp
gcc -O0 -g -gsplit-dwarf -c ../main.c ../util.c
ls -l *.o *.dwo
readelf -SW util.o | grep -E "debug|Name"
readelf -SW util.dwo | grep -E "debug"
gcc main.o util.o -o sp
readelf --debug-dump=info sp | grep -E "dwo_name|comp_dir|DW_TAG"
eval $G ./sp 2>&1
# GNU dwp 2.44 两种方式都退出成功，但 GDB 17.1 读这组 DWARF 5 包时报 hash table 错误
dwp -e sp -o sp.dwp || echo "dwp 退出码 $?"
dwp -o sp.dwp main.dwo util.dwo && ls -l sp.dwp
mkdir -p ../hide && mv *.dwo ../hide/
eval $G ./sp 2>&1 || true
rm -f sp.dwp
eval $G ./sp 2>&1 || true
# 对照：同一套工具改用 DWARF 4 打包后，GDB 能断点和回溯
rm -f *.o sp sp.dwp ../hide/*
gcc -O0 -g -gdwarf-4 -gsplit-dwarf -c ../main.c ../util.c
gcc main.o util.o -o sp
dwp -e sp -o sp.dwp && rm *.dwo
ls -l sp sp.dwp
eval $G ./sp 2>&1
# 练习三：CRC 对不上，以及重写 debuglink 之后
cd .. && mkdir -p ex && find ex -mindepth 1 -delete && cd ex
gcc -O0 -g ../main.c ../util.c -o prog
objcopy --only-keep-debug prog prog.debug
objcopy --strip-debug --add-gnu-debuglink=prog.debug prog prog.stripped
printf x >> prog.debug
eval $V ./prog.stripped 2>&1 || true
objcopy --remove-section=.gnu_debuglink prog.stripped
objcopy --add-gnu-debuglink=prog.debug prog.stripped
eval $V ./prog.stripped 2>&1
# 编译和链接一步完成时 .dwo 的名字
cd .. && mkdir -p one && find one -mindepth 1 -delete && cd one
gcc -O0 -g -gsplit-dwarf ../main.c ../util.c -o sp2
ls
readelf --debug-dump=info sp2 | grep -E "dwo_name"
# 练习三：另一次构建的调试文件也能骗过 debuglink
cd .. && mkdir -p mm && find mm -mindepth 1 -delete && cd mm
gcc -O0 -g ../main.c ../util.c -o prog
sed "s/a + b/a + b + 1/" ../util.c > util2.c
gcc -O0 -g ../main.c util2.c -o other
objcopy --only-keep-debug other prog.debug
objcopy --strip-debug prog prog.stripped
objcopy --add-gnu-debuglink=prog.debug prog.stripped
readelf -n prog.stripped other | grep "Build ID"
eval $V ./prog.stripped 2>&1
)
