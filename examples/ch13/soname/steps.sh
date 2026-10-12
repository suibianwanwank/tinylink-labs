set -ex
# 1) libtool 的 current:revision:age 换算成文件名和 soname
libtool --version | head -1
libtool --mode=compile gcc -O1 -c demo.c >/dev/null
for v in 0:0:0 1:0:1 1:1:1 2:0:0 3:1:2; do
  rm -f .libs/libdemo.so* libdemo.la
  libtool --mode=link gcc -o libdemo.la demo.lo -rpath /usr/local/lib -version-info $v >/dev/null
  echo "== -version-info $v: $(ls .libs | grep '^libdemo.so' | tr '\n' ' ')"
  readelf -d .libs/libdemo.so | grep SONAME
done
# 2) 三个名字：真实文件、soname 链接、开发用链接
mkdir -p lib && find lib -mindepth 1 -delete
gcc -O1 -fPIC -shared -DPOINT_H='"point1.h"' -Wl,-soname,libpoint.so.1 point.c -o lib/libpoint.so.1.0.0
gcc -O1 -fPIC -shared -DPOINT_H='"point2.h"' -DHAVE_Z -Wl,-soname,libpoint.so.2 point.c -o lib/libpoint.so.2.0.0
ldconfig -n -v lib 2>/dev/null
ln -sf libpoint.so.1 lib/libpoint.so
gcc -O1 -DPOINT_H='"point1.h"' app.c -Llib -lpoint -Wl,-rpath,$W/soname/lib -o app1
ln -sf libpoint.so.2 lib/libpoint.so
gcc -O1 -DPOINT_H='"point2.h"' app.c -Llib -lpoint -Wl,-rpath,$W/soname/lib -o app2
ls -l lib | awk '{print $1, $9, $10, $11}'
readelf -d app1 | grep NEEDED | head -1
readelf -d app2 | grep NEEDED | head -1
./app1; ./app2
LD_DEBUG=libs ./app1 2>&1 | grep 'trying file=.*libpoint' | tail -1
LD_DEBUG=libs ./app2 2>&1 | grep 'trying file=.*libpoint' | tail -1
# 3) 发行版包名与实际 SONAME 都从当前机器读取。
for p in /usr/lib/x86_64-linux-gnu/libstdc++.so.6 /usr/lib/x86_64-linux-gnu/libc.so.6 /usr/lib/x86_64-linux-gnu/libabigail.so.*; do
 [ -f "$p" ] || continue
 dpkg-query -S "$p" || true
 readelf -d "$p" | grep SONAME
done
mkdir -p "$W/debs"; cd "$W/debs"
apt-get download -qq libssl3t64 libssl-dev
dpkg -c libssl3t64_*.deb | grep 'libssl\.so'
dpkg -c libssl-dev_*.deb | grep 'libssl\.so'
dpkg -I libssl-dev_*.deb | grep Depends
