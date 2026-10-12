# 练习二：预测
set -ex
libtool --mode=compile gcc -O1 -c demo.c >/dev/null
for v in 4:2:1 4:3:1 5:0:2 6:0:0; do
  rm -f .libs/libfoo.so* libfoo.la
  libtool --mode=link gcc -o libfoo.la demo.lo -rpath /usr/local/lib -version-info $v >/dev/null
  echo "$v: $(ls .libs | grep '^libfoo.so.*\..*\.' ) soname=$(readelf -d .libs/libfoo.so | sed -n 's/.*soname: \[\(.*\)\]/\1/p')"
done
# (2) 没有版本信息的引用碰上带版本的库
for d in v0 v2; do mkdir -p "$d" && find "$d" -mindepth 1 -delete; done
gcc -O1 -fPIC -shared -Wl,-soname,libparse.so.1 parse1.c -o v0/libparse.so.1      # 不加版本脚本
gcc -O1 -fPIC -shared -Wl,-soname,libparse.so.1 -Wl,--version-script=parse2.map parse2.c -o v2/libparse.so.1
gcc -O1 app.c v0/libparse.so.1 -o app_unver
readelf --dyn-syms -W app_unver | grep parse_num
readelf -V app_unver | grep -A1 'File: libparse' || echo "(没有对 libparse.so.1 的版本需求)"
LD_LIBRARY_PATH=v2 ./app_unver
LD_DEBUG=bindings LD_LIBRARY_PATH=v2 ./app_unver 2>&1 | grep "symbol \`parse_num"
