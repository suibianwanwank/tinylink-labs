set -x
mkdir -p v1 v2
gcc -g -O1 -fPIC -shared -DPOINT_H='"point1.h"' -Wl,-soname,libpoint.so.1 point.c -o v1/libpoint.so.1
gcc -g -O1 -fPIC -shared -DPOINT_H='"point2.h"' -DHAVE_Z -Wl,-soname,libpoint.so.1 point.c -o v2/libpoint.so.1
gcc -g -O1 -fPIC -shared -Wl,-soname,libcolor.so.1 color.c -o v1/libcolor.so.1
gcc -g -O1 -fPIC -shared -Wl,-soname,libcolor.so.1 -DV2 color.c -o v2/libcolor.so.1
abidiff v1/libpoint.so.1 v2/libpoint.so.1; echo "abidiff exit=$?"
abidiff v1/libcolor.so.1 v2/libcolor.so.1; echo "abidiff exit=$?"
# 去掉调试信息之后，abidiff 只剩符号表可比
strip -o v1/libpoint.stripped.so v1/libpoint.so.1; strip -o v2/libpoint.stripped.so v2/libpoint.so.1
abidiff v1/libpoint.stripped.so v2/libpoint.stripped.so; echo "abidiff exit=$?"
# abi-compliance-checker：先用 abi-dumper 从调试信息生成转储，再比较
abi-dumper v1/libpoint.so.1 -o v1.dump -lver 1.0 >/dev/null
abi-dumper v2/libpoint.so.1 -o v2.dump -lver 1.1 >/dev/null
timeout --kill-after=2s 40s abi-compliance-checker -l libpoint -old v1.dump -new v2.dump -report-path report.html; echo "acc exit=$?"
# -strict：把警告也算作问题
timeout --kill-after=2s 40s abi-compliance-checker -l libpoint -old v1.dump -new v2.dump -strict -report-path report-strict.html; echo "acc -strict exit=$?"
# 按 abi-dumper 的提示换成 -Og 重编再比一次
mkdir -p og1 og2
gcc -g -Og -fPIC -shared -DPOINT_H='"point1.h"' -Wl,-soname,libpoint.so.1 point.c -o og1/libpoint.so.1
gcc -g -Og -fPIC -shared -DPOINT_H='"point2.h"' -DHAVE_Z -Wl,-soname,libpoint.so.1 point.c -o og2/libpoint.so.1
abi-dumper og1/libpoint.so.1 -o og1.dump -lver 1.0 >/dev/null; abi-dumper og2/libpoint.so.1 -o og2.dump -lver 1.1 >/dev/null
timeout --kill-after=2s 40s abi-compliance-checker -l libpoint -old og1.dump -new og2.dump -report-path report-og.html > og.txt; echo "acc -Og exit=$?"; grep compatibility og.txt
timeout --kill-after=2s 40s abi-compliance-checker -l libpoint -old og1.dump -new og2.dump -strict -report-path report-og-strict.html | grep compatibility
# 用法错误的退出码
abidiff --no-such-option v1/libpoint.so.1 v2/libpoint.so.1 >/dev/null 2>&1; echo "abidiff usage error exit=$?"
# 内部函数默认导出：它的变化也算进 ABI
gcc -g -O1 -fPIC -shared fmt.c -o v1/libfmt.so
gcc -g -O1 -fPIC -shared -DV2 fmt.c -o v2/libfmt.so
abidiff v1/libfmt.so v2/libfmt.so; echo "abidiff exit=$?"
# -fvisibility=hidden 加显式导出宏：fmt_pad 不再是 ABI 的一部分
gcc -g -O1 -fPIC -shared -fvisibility=hidden -DEXPORT_MACRO fmt.c -o v1/libfmt_h.so
gcc -g -O1 -fPIC -shared -fvisibility=hidden -DEXPORT_MACRO -DV2 fmt.c -o v2/libfmt_h.so
readelf --dyn-syms -W v2/libfmt.so | grep fmt_; readelf --dyn-syms -W v2/libfmt_h.so | grep fmt_
abidiff v1/libfmt_h.so v2/libfmt_h.so; echo "abidiff exit=$?"
# 虚函数表里的位置变了：abidiff 能断定不兼容，退出码再加 8
g++ -g -O1 -fPIC -shared -Wl,-soname,libshape.so.1 shape.cc -o v1/libshape.so.1
g++ -g -O1 -fPIC -shared -Wl,-soname,libshape.so.1 -DV2 shape.cc -o v2/libshape.so.1
abidiff v1/libshape.so.1 v2/libshape.so.1; echo "abidiff exit=$?"
# 版本脚本 local: *; 效果相同
gcc -g -O1 -fPIC -shared -Wl,--version-script=fmt.map fmt.c -o v1/libfmt_m.so
gcc -g -O1 -fPIC -shared -Wl,--version-script=fmt.map -DV2 fmt.c -o v2/libfmt_m.so
readelf --dyn-syms -W v2/libfmt_m.so | grep fmt_
abidiff v1/libfmt_m.so v2/libfmt_m.so; echo "abidiff exit=$?"
