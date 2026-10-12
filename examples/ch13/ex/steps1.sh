# 练习一：观察
set -e
F=$W/files.txt
for f in /usr/bin/* /usr/lib/x86_64-linux-gnu/*.so.*; do
  [ -L "$f" ] || [ ! -f "$f" ] && continue
  objdump -T "$f" >/dev/null 2>&1 && echo "$f"
done > $F
# (1a) 天真的口径：objdump -T 里出现的最高 GLIBC 版本
naive() { objdump -T "$1" 2>/dev/null | grep -oE 'GLIBC_2(\.[0-9]+)+' | sort -uV | tail -1; }
# (1b) 只数 *UND*，也就是这个文件引用的版本
need()  { objdump -T "$1" 2>/dev/null | awk '/\*UND\*/' | grep -oE 'GLIBC_2(\.[0-9]+)+' | sort -uV | tail -1; }
for f in $(cat $F); do v=$(naive $f); [ -n "$v" ] && echo "$v $f"; done | sort -V > $W/naive.txt
for f in $(cat $F); do v=$(need $f);  [ -n "$v" ] && echo "$v $f"; done | sort -V > $W/need.txt
echo "== 天真口径：$(wc -l < $W/naive.txt) 个文件"; cut -d' ' -f1 $W/naive.txt | uniq -c
echo "== 只数 *UND*：$(wc -l < $W/need.txt) 个文件"; cut -d' ' -f1 $W/need.txt | uniq -c
echo "== 两种口径不一致的文件"
join -1 2 -2 2 -a 1 -e '-' -o 0,1.1,2.1 <(sort -k2 $W/naive.txt) <(sort -k2 $W/need.txt) | awk '$2 != $3'
# (1c) 最高需求停在 2.34 的文件：是哪些 2.34 符号把它推上去的
echo "== 最高需求是 GLIBC_2.34 的文件，按引用的 2.34 符号计数"
for f in $(awk '$1=="GLIBC_2.34"{print $2}' $W/need.txt); do
  case $f in /usr/bin/*) k=bin;; *) k=lib;; esac
  objdump -T $f | awk -v k=$k '/\*UND\*/ && /\(GLIBC_2\.34\)/ {print k, $NF}'
done | sort | uniq -c | sort -rn | head -10
echo "== 最高需求是 GLIBC_2.36 的文件"
for f in $(awk '$1=="GLIBC_2.36"{print $2}' $W/need.txt); do echo "$f: $(objdump -T $f | awk '/\*UND\*/ && /\(GLIBC_2\.36\)/ {print $NF}' | tr '\n' ' ')"; done
# (2) libstdc++ 的版本节点
L=/usr/lib/x86_64-linux-gnu/libstdc++.so.6
readelf -V $L | grep -c 'Name: GLIBCXX_'
readelf -V $L | grep -c 'Name: CXXABI_'
readelf -V $L | grep -A1 'Name: GLIBCXX_3.4.30$'
readelf --dyn-syms -W $L | grep -c '@@GLIBCXX_3.4.30'
readelf --dyn-syms -W $L | grep '@@GLIBCXX_3.4.30' | grep _M_extract_via_format
