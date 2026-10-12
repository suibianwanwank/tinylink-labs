set -ex
L=/lib/x86_64-linux-gnu/libc.so.6
ldd --version | head -1
cp $L libc-native.so   # 练习四的 verdump 拿它对拍
# aarch64 的 glibc：memcpy 只有一个版本，fmemopen、pthread_create 各有两个
readelf --dyn-syms -W $L | grep -E ' (memcpy|memmove|fmemopen|pthread_create|__libc_start_main)@'
readelf -V $L | grep -c 'Name: GLIBC_'
readelf -V $L | grep 'Name: GLIBC_' | head -3
# fmemopen：默认绑新实现，用 .symver 绑旧实现
gcc -O1 fm.c -o fm_new
gcc -O1 -DOLD fm.c -o fm_old
objdump -T fm_new fm_old | grep fmemopen
./fm_new; ./fm_old
# 在 glibc 2.36 上编译的程序要哪些版本
gcc -O1 th.c -o th
objdump -T th | grep GLIBC_
objdump -T th | grep -o 'GLIBC_[0-9.]*' | sort -uV | tail -1
# 把 pthread_create 钉到旧版本，还剩谁要 GLIBC_2.34
gcc -O1 -DOLD_PTHREAD th.c -o th_pin
objdump -T th_pin | grep GLIBC_2.34
nm /usr/lib/x86_64-linux-gnu/Scrt1.o | grep libc_start
# 新头文件里 stat 是导出函数，2.33 起才有
gcc -O1 st.c -o st && objdump -T st | grep stat
grep -n -A4 '^__NTH (stat (const char' $W/sysroot-el7/usr/include/sys/stat.h
# 在旧 sysroot（CentOS 7 的 glibc 2.17）上链接
S=$W/sysroot-el7
gcc -O1 --sysroot=$S -B$S/usr/lib64 -L$S/usr/lib64 -L$S/lib64 th.c -lpthread -o th_el7
readelf -h th_el7 | grep Type
objdump -T th_el7 | grep GLIBC_
readelf -d th_el7 | grep NEEDED
./th_el7
# 2.34 之后保留的空壳 libpthread.so.0
readelf --dyn-syms -W /lib/x86_64-linux-gnu/libpthread.so.0 | grep -c FUNC
