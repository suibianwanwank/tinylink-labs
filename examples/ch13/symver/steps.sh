set -ex
for d in v1 v2; do mkdir -p "$d" && find "$d" -mindepth 1 -delete; done
gcc -O1 -fPIC -shared -Wl,-soname,libparse.so.1 -Wl,--version-script=parse1.map parse1.c -o v1/libparse.so.1
gcc -O1 -fPIC -shared -Wl,-soname,libparse.so.1 -Wl,--version-script=parse2.map parse2.c -o v2/libparse.so.1
ln -sf libparse.so.1 v1/libparse.so; ln -sf libparse.so.1 v2/libparse.so
# 2 版库：两个 parse_num，一个默认
readelf --dyn-syms -W v2/libparse.so.1 | grep parse_
readelf -V v2/libparse.so.1 | sed -n '/version_d/,$p'
# 旧程序在 1 版上链接，新程序在 2 版上链接
gcc -O1 app.c -Lv1 -lparse -o app_old
gcc -O1 app.c -Lv2 -lparse -o app_new
readelf --dyn-syms -W app_old | grep parse_num
readelf --dyn-syms -W app_new | grep parse_num
readelf -V app_new | sed -n '/version_r/,$p'
# 新编译的程序也可以显式请求非默认的旧版本
gcc explicit-old.c -Lv2 -lparse -o explicit-old
LD_LIBRARY_PATH=v2 ./explicit-old
# 两个程序都在 2 版上运行
LD_LIBRARY_PATH=v2 ./app_old
LD_LIBRARY_PATH=v2 ./app_new
LD_DEBUG=bindings LD_LIBRARY_PATH=v2 ./app_old 2>&1 | grep "symbol \`parse_num"
LD_DEBUG=bindings LD_LIBRARY_PATH=v2 ./app_new 2>&1 | grep "symbol \`parse_num"
# 新程序放到只有 1 版的机器上
LD_LIBRARY_PATH=v1 ./app_new || echo "exit=$?"
