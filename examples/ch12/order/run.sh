#!/bin/sh
# Linux 原生运行、交错计时；PMU 若不可用，不把模拟计数当硬件数据。
set -e
. "$(dirname "$0")/../env.sh"
HERE=$(cd "$(dirname "$0")" && pwd)
TAG=-big N=65536 STRIDE=16 ITER=4000 sh "$HERE/build.sh"
(cd "$W/order-big"; python3 - <<'MEASURE'
import subprocess,time,statistics,resource
rows={p:[] for p in ['p_default','p_ordered']}
for i in range(8):
 for p in (['p_default','p_ordered'] if i%2==0 else ['p_ordered','p_default']):
  start=time.perf_counter(); result=subprocess.run(['./'+p],text=True,capture_output=True); assert result.returncode == 79, result; out=result.stdout.strip(); rows[p].append(time.perf_counter()-start)
  print(p,out,rows[p][-1])
for p,v in rows.items():print(p,'median_seconds',statistics.median(v))
MEASURE
)
# 冷热属性与 -z keep-text-section-prefix
mkdir -p "$W/hotcold" && cp "$HERE/hotcold.c" "$W/hotcold/" && cd "$W/hotcold"
set -x
$CC -O2 -ffunction-sections -c hotcold.c -o hc_clang.o
llvm-readelf -SW hc_clang.o | grep -E '\.text'
gcc -O2 -c hotcold.c -o hc_gcc.o
llvm-readelf -SW hc_gcc.o | grep -E '\.text'
for o in hc_clang.o hc_gcc.o; do
  ld.lld -e main $o -o $o.out
  ld.lld -e main -z keep-text-section-prefix $o -o $o.keep.out
  llvm-readelf -SW $o.out | grep -E '\.text'
  llvm-readelf -SW $o.keep.out | grep -E '\.text'
  llvm-nm -n $o.keep.out
done
ld.bfd -e main hc_gcc.o -o bfd.out
llvm-nm -n bfd.out | grep -E ' [Tt] '
ld.bfd --verbose | grep -E 'text\.(unlikely|hot|startup)'
