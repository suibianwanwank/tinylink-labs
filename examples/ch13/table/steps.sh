set -ex
mkdir -p v1 v2
# enum 中间插入一项
gcc -O1 -fPIC -shared -Wl,-soname,libcolor.so.1 color.c -o v1/libcolor.so.1
gcc -O1 -fPIC -shared -Wl,-soname,libcolor.so.1 -DV2 color.c -o v2/libcolor.so.1
gcc -O1 color_app.c v1/libcolor.so.1 -o color_app
LD_LIBRARY_PATH=v1 ./color_app
LD_LIBRARY_PATH=v2 ./color_app
# 头文件里的内联函数改了实现
gcc -O1 -fPIC -shared -Wl,-soname,libtab.so.1 tab.c -o v1/libtab.so.1
gcc -O1 -fPIC -shared -Wl,-soname,libtab.so.1 -DV2 tab.c -o v2/libtab.so.1
gcc -O1 tab_app.c v1/libtab.so.1 -o tab_app
LD_LIBRARY_PATH=v1 ./tab_app
LD_LIBRARY_PATH=v2 ./tab_app
nm -D tab_app | grep tab_
# C++：在已有虚函数前面加一个虚函数
g++ -O1 -fPIC -shared -Wl,-soname,libshape.so.1 shape.cc -o v1/libshape.so.1
g++ -O1 -fPIC -shared -Wl,-soname,libshape.so.1 -DV2 shape.cc -o v2/libshape.so.1
g++ -O1 shape_app.cc v1/libshape.so.1 -o shape_app
LD_LIBRARY_PATH=v1 ./shape_app
LD_LIBRARY_PATH=v2 ./shape_app
objdump -d --no-show-raw-insn -C shape_app | sed -n '/<main>:/,/ret/p' | grep -E 'mov|call'
# delete s 走的是虚表第 2 项：1.1 里那一项变成了不释放内存的析构函数
readelf -r -W v1/libshape.so.1 | grep -E 'R_X86_64_64 .*_ZN?K?5Shape'
readelf -r -W v2/libshape.so.1 | grep -E 'R_X86_64_64 .*_ZN?K?5Shape'
g++ -O1 shape_leak.cc v1/libshape.so.1 -o shape_leak
LD_LIBRARY_PATH=v1 ./shape_leak
LD_LIBRARY_PATH=v2 ./shape_leak
