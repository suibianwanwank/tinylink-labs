# expect: exit 0
# 和 three-modules.tl 是同一个程序：换行、缩进、Tab 都只是分隔符。
module main def limit 3 use print limit text I 5000 E 1000 E 2001 A 9007 end	module print
def print 1 use buf text R 7000
		E 6000 R 8001 end module
data def buf 0 text I 0 I 0 end   # 注释可以跟在任何记号后面
