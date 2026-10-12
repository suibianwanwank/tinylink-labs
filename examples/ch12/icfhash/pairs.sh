#!/bin/sh
# 把 --print-icf-sections 格式的输出变成排序后的"保留节 被删节"对，只留文件名匹配 $1、
# 节名以 .text. 开头的那些。lld 输出各组的顺序不是输入顺序，所以只能比较排序后的对。
awk -v pat="$1" '
  /^selected section / { kept = $3; next }
  /^  removing identical section / {
    r = $4
    if (index(kept, pat) == 1 && index(r, pat) == 1 && kept ~ /:\(\.text\./ && r ~ /:\(\.text\./)
      print kept, r
  }' | sort
