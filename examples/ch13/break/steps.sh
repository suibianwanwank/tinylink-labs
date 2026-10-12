set -ex
# 1.0 版的库和用它编译的程序
mkdir -p v1 v2
gcc -g -O1 -fPIC -shared -DPOINT_H='"point1.h"' -Wl,-soname,libpoint.so.1 point.c -o v1/libpoint.so.1
ln -sf libpoint.so.1 v1/libpoint.so
gcc -O1 -DPOINT_H='"point1.h"' app.c -Lv1 -lpoint -o app
LD_LIBRARY_PATH=v1 ./app
# 1.1 版：结构体加字段，soname 不变
gcc -g -O1 -fPIC -shared -DPOINT_H='"point2.h"' -DHAVE_Z -Wl,-soname,libpoint.so.1 point.c -o v2/libpoint.so.1
ln -sf libpoint.so.1 v2/libpoint.so
# 旧程序不重新编译，换上新库
LD_LIBRARY_PATH=v2 ./app
# 用新头文件重新编译，结果又对了
gcc -O1 -DPOINT_H='"point2.h"' app.c -Lv2 -lpoint -o app_new
LD_LIBRARY_PATH=v2 ./app_new
# 动态链接器检查过什么
readelf --dyn-syms -W v2/libpoint.so.1 | grep point_
readelf -d app | grep NEEDED
