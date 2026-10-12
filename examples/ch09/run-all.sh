#!/bin/sh
# 在原生 x86-64 Linux 编译和检查 TLS 模型；AArch64 只作文件/指令对照。
# 运行练习由 ex/run-linux.sh 单独完成，不执行只有 ret 的占位入口。
set -e
[ "$(uname -s)" = Linux ] && [ "$(uname -m)" = x86_64 ]
D=$(cd "$(dirname "$0")" && pwd)
if [ -n "${W:-}" ]; then
  mkdir "$W" || { echo "W must name a new experiment directory" >&2; exit 1; }
else
  W=$(mktemp -d "${TMPDIR:-/tmp}/linker-ch09.XXXXXX")
fi
printf 'experiment directory: %s\n' "$W"
cp "$D"/tls.c "$D"/ext.c "$D"/ld.c "$D"/start.s "$W/"
cd "$W"
X="clang -O1"
A="clang --target=aarch64-unknown-linux-gnu -O1"
RE=${RE:-readelf}
LD_BFD=${LD_BFD:-ld.bfd}
OD=${OD:-llvm-objdump}
$X -c start.s -o start.o
$X -fPIC -c ext.c -o ext.o
$X -fPIC -c tls.c -o pic.o
$X -fPIC -c ld.c -o ld.o
$X -c tls.c -o nopic.o
$X -fPIC -ftls-model=initial-exec -c tls.c -o ie.o
$X -fPIC -mtls-dialect=gnu2 -c tls.c -o desc.o
$A -c start.s -o a64start.o
$A -fPIC -c ext.c -o a64ext.o
$A -fPIC -c tls.c -o a64pic.o
ld.lld -pie start.o pic.o ext.o -o gd2le
ld.lld -pie start.o nopic.o ext.o -o ie2le
reject_local_exec() {
  label=$1; shift
  if "$@" > "$label.log" 2>&1; then
    echo "unexpected success: $*" >&2; exit 1
  fi
  grep -q R_X86_64_TPOFF32 "$label.log" || { cat "$label.log" >&2; exit 1; }
  sed "s/^/$label: /" "$label.log"
}
reject_local_exec x.so ld.lld -shared nopic.o -o x.so
reject_local_exec y.so "$LD_BFD" -shared nopic.o -o y.so
ld.lld -shared pic.o -o libtls.so
ld.lld -shared -soname libext.so ext.o -o libext.so
ld.lld -pie start.o pic.o libext.so -o gd2ie
ld.lld -pie start.o ld.o -o ld2le
ld.lld -shared desc.o -o libdesc.so
ld.lld -pie a64start.o a64pic.o a64ext.o -o a64exe
ld.lld -shared ie.o -o libie.so
$LD_BFD -shared ie.o -o libie_bfd.so
echo "== PT_TLS (gd2le)"; $RE -lW gd2le | grep TLS
echo "== 重定位类型"; for f in pic.o ld.o nopic.o ie.o desc.o a64pic.o; do
  printf '%-9s ' $f; $RE -rW $f | awk '/R_/{print $3}' | sort -u | tr '\n' ' '; echo; done
echo "== 动态重定位"; for f in libtls.so gd2ie libdesc.so; do
  printf '%-10s ' $f; $RE -rW $f | awk '/R_/{print $3}' | sort | uniq -c | tr -s ' ' | tr '\n' ' '; echo; done
echo "== 降级后还剩的 __tls_get_addr 调用"; for f in gd2le ld2le gd2ie; do
  printf '%-6s %s\n' $f "$("$OD" -d $f | grep -c '__tls_get_addr' || true)"; done
echo "== DT_FLAGS"; for f in libie.so libie_bfd.so; do printf '%-13s ' $f; $RE -dW $f | grep FLAGS | tr -s ' ' | tr '\n' ' '; echo; done
echo "== 第 9 章全部命令执行完毕"
