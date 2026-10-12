# 练习三：改坏
set -x
for d in v1 v3a v3b; do mkdir -p "$d" && find "$d" -mindepth 1 -delete; done
gcc -O1 -fPIC -shared -Wl,-soname,libparse.so.1 -Wl,--version-script=../symver/parse1.map parse1.c -o v1/libparse.so.1
gcc -O1 app.c v1/libparse.so.1 -o app_old
gcc -O1 -fPIC -shared -Wl,-soname,libparse.so.1 -Wl,--version-script=parse3a.map parse3.c -o v3a/libparse.so.1
gcc -O1 -fPIC -shared -Wl,-soname,libparse.so.1 -Wl,--version-script=parse3b.map parse3.c -o v3b/libparse.so.1
LD_LIBRARY_PATH=v3a ./app_old; echo "exit=$?"
LD_LIBRARY_PATH=v3b ./app_old; echo "exit=$?"
LD_BIND_NOW=1 LD_LIBRARY_PATH=v3b ./app_old; echo "exit=$?"
readelf --dyn-syms -W v3b/libparse.so.1 | grep -E 'parse|VERS'
# 报错发生在什么时候：main 一开始先打印一行
gcc -O1 -Wl,-z,lazy app_start.c v1/libparse.so.1 -o app_start
LD_LIBRARY_PATH=v3b ./app_start 2>&1; echo "exit=$?"
LD_BIND_NOW=1 LD_LIBRARY_PATH=v3b ./app_start 2>&1; echo "exit=$?"
LD_LIBRARY_PATH=v3a ./app_start 2>&1; echo "exit=$?"
# 修好：把旧实现和 VERS_1 都留着，就是 2 版的做法
mkdir -p v3fix
gcc -O1 -fPIC -shared -Wl,-soname,libparse.so.1 -Wl,--version-script=parse2.map parse2.c -o v3fix/libparse.so.1
LD_LIBRARY_PATH=v3fix ./app_old; echo "exit=$?"
