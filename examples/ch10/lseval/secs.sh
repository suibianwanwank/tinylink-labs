#!/bin/sh
# 用法：secs.sh a.o b.o ...
# 按命令行顺序输出两类行：
#   SEC 文件 节名 大小 对齐 类型   可分配输入节（外加没有标志的 trampsec）
#   SYM 文件 D|U 符号名            全局符号的定义（D）与未定义引用（U）
for f in "$@"; do
  riscv64-unknown-elf-readelf -SW "$f" | sed -n 's/^ *\[ *[0-9]*\] //p' | while read name type addr off size es flg rest; do
    case "$flg$name" in *A*|*trampsec) ;; *) continue;; esac
    al=$(echo "$rest" | awk '{print $NF}')
    echo "SEC $f $name $((0x$size)) $al $type"
  done
  riscv64-unknown-elf-nm -g "$f" | awk -v f="$f" 'NF==2 && $1=="U" {print "SYM", f, "U", $2} NF==3 {print "SYM", f, "D", $3}'
done
